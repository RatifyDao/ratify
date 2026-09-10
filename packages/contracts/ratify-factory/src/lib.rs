#![no_std]
//! # Ratify factory
//!
//! Deploys a community's six contracts in one transaction, wires their
//! permissions, and keeps the register of every community deployed.
//!
//! The permission chain is only worth anything if it is set up correctly, and
//! setting it up by hand across six contracts is exactly the kind of job that
//! goes wrong quietly. Here it happens in one transaction that either works or
//! reverts, and the addresses are computed before anything is deployed, so
//! contracts that need each other's address can both be constructed with it.
//!
//! When the transaction ends:
//!
//! * the timelock takes scheduling from the governor and nobody else
//! * the treasury takes instructions from the timelock and nobody else
//! * the delegate registry takes writes from the governor and nobody else
//! * the guardian can cancel a queued action and can do nothing else
//! * the governor holds no funds and cannot reach the treasury
//!
//! The one thing the factory leaves in a human's hands is issuing membership,
//! which starts with the founder. A community with no members cannot govern
//! itself into existence, so somebody has to admit the first ones. Moving
//! issuance under the timelock afterwards is a single call the founder makes,
//! and until they make it the register shows that they have not.

mod types;

#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contractimpl, panic_with_error, Address, Bytes, BytesN, Env, IntoVal, Val, Vec,
};

pub use crate::types::{
    Community, CommunityDeployed, FactoryError, FactoryStorageKey, Settings, TenureSchedule, Wasms,
    WeightModel,
};

/// The most communities one register will hold.
///
/// A register that grows without limit is a page that eventually cannot be
/// loaded. Ten thousand is more than any single deployment will see, and it
/// makes the bound explicit rather than discovered.
pub const MAX_COMMUNITIES: u32 = 10_000;

const EXTEND_AMOUNT: u32 = 30 * 17_280;
const TTL_THRESHOLD: u32 = EXTEND_AMOUNT - 17_280;

#[contract]
pub struct RatifyFactory;

#[contractimpl]
impl RatifyFactory {
    /// Fixes the code every community from this factory will run.
    ///
    /// The hashes cannot be changed. A factory that could be repointed at
    /// different code would mean the register's guarantee was only as good as
    /// whoever held the key, and there is no key.
    pub fn __constructor(e: &Env, wasms: Wasms) {
        e.storage()
            .instance()
            .set(&FactoryStorageKey::Wasms, &wasms);
        e.storage().instance().set(&FactoryStorageKey::Count, &0u32);
    }

    // ################## THE REGISTER ##################

    /// Returns the wasm every community from this factory runs.
    pub fn wasms(e: &Env) -> Wasms {
        e.storage()
            .instance()
            .get(&FactoryStorageKey::Wasms)
            .unwrap_or_else(|| panic_with_error!(e, FactoryError::NotConfigured))
    }

    /// Returns how many communities the register holds.
    pub fn community_count(e: &Env) -> u32 {
        e.storage()
            .instance()
            .get(&FactoryStorageKey::Count)
            .unwrap_or(0)
    }

    /// Returns a community by its position in the register.
    pub fn community(e: &Env, index: u32) -> Community {
        e.storage()
            .persistent()
            .get(&FactoryStorageKey::Community(index))
            .unwrap_or_else(|| panic_with_error!(e, FactoryError::NotFound))
    }

    /// Returns a community by its governor, which is the address a member
    /// arrives with.
    pub fn community_by_governor(e: &Env, governor: Address) -> Option<Community> {
        let index: u32 = e
            .storage()
            .persistent()
            .get(&FactoryStorageKey::IndexOfGovernor(governor))?;
        e.storage()
            .persistent()
            .get(&FactoryStorageKey::Community(index))
    }

    /// Returns a page of the register, oldest first.
    ///
    /// The directory reads this. A `start` past the end returns nothing rather
    /// than failing, so paging off the end of the register is not an error.
    pub fn communities(e: &Env, start: u32, limit: u32) -> Vec<Community> {
        let count = Self::community_count(e);
        let mut page: Vec<Community> = Vec::new(e);
        if start >= count {
            return page;
        }
        let end = start.saturating_add(limit).min(count);
        for index in start..end {
            if let Some(community) = e
                .storage()
                .persistent()
                .get::<_, Community>(&FactoryStorageKey::Community(index))
            {
                page.push_back(community);
            }
        }
        page
    }

    // ################## DEPLOYING ##################

    /// Returns the addresses a community deployed under `salt` will have.
    ///
    /// Computed the same way the deployment computes them, so a founder can
    /// see where their community will live before they pay for it, and so the
    /// governor's address can be handed to the timelock that has to trust it.
    pub fn addresses_for(
        e: &Env,
        salt: BytesN<32>,
    ) -> (Address, Address, Address, Address, Address, Address) {
        (
            Self::address_for(e, &salt, "membership"),
            Self::address_for(e, &salt, "weightrule"),
            Self::address_for(e, &salt, "governor"),
            Self::address_for(e, &salt, "timelock"),
            Self::address_for(e, &salt, "treasury"),
            Self::address_for(e, &salt, "registry"),
        )
    }

    /// Deploys a community and enters it in the register.
    ///
    /// Either every contract is deployed and wired, or the transaction
    /// reverts and none of them are. There is no half-built community.
    pub fn deploy(e: &Env, salt: BytesN<32>, settings: Settings) -> Community {
        settings.founder.require_auth();

        if settings.timelock_delay == 0 {
            panic_with_error!(e, FactoryError::DelayCannotBeZero);
        }
        if settings.voting_period == 0 {
            panic_with_error!(e, FactoryError::VotingPeriodCannotBeZero);
        }
        if settings.quorum_bps == 0 || settings.quorum_bps > 10_000 {
            panic_with_error!(e, FactoryError::InvalidQuorum);
        }

        let index = Self::community_count(e);
        if index >= MAX_COMMUNITIES {
            panic_with_error!(e, FactoryError::RegisterFull);
        }

        let wasms = Self::wasms(e);

        // Every address is known before anything is deployed, which is what
        // lets the timelock be built already trusting the governor, and the
        // registry already trusting it too.
        let membership = Self::address_for(e, &salt, "membership");
        let weight_rule = Self::address_for(e, &salt, "weightrule");
        let governor = Self::address_for(e, &salt, "governor");
        let timelock = Self::address_for(e, &salt, "timelock");
        let treasury = Self::address_for(e, &salt, "treasury");
        let registry = Self::address_for(e, &salt, "registry");

        Self::deploy_at(
            e,
            &salt,
            "membership",
            &wasms.membership,
            (
                settings.founder.clone(),
                settings.name.clone(),
                settings.symbol.clone(),
                settings.base_uri.clone(),
            )
                .into_val(e),
        );

        Self::deploy_at(
            e,
            &salt,
            "weightrule",
            &wasms.weight_rule,
            (
                membership.clone(),
                WeightModel::OneTokenOneVote,
                Option::<TenureSchedule>::None,
            )
                .into_val(e),
        );

        Self::deploy_at(
            e,
            &salt,
            "timelock",
            &wasms.timelock,
            (
                governor.clone(),
                settings.guardian.clone(),
                settings.timelock_delay,
            )
                .into_val(e),
        );

        Self::deploy_at(
            e,
            &salt,
            "treasury",
            &wasms.treasury,
            (timelock.clone(),).into_val(e),
        );

        Self::deploy_at(
            e,
            &salt,
            "registry",
            &wasms.registry,
            (
                governor.clone(),
                weight_rule.clone(),
                settings.contested_margin_bps,
            )
                .into_val(e),
        );

        Self::deploy_at(
            e,
            &salt,
            "governor",
            &wasms.governor,
            (
                weight_rule.clone(),
                timelock.clone(),
                Some(registry.clone()),
                settings.name.clone(),
                settings.voting_delay,
                settings.voting_period,
                settings.proposal_threshold,
                settings.quorum_bps,
            )
                .into_val(e),
        );

        let community = Community {
            membership: membership.clone(),
            weight_rule: weight_rule.clone(),
            governor: governor.clone(),
            timelock: timelock.clone(),
            treasury: treasury.clone(),
            registry: registry.clone(),
            name: settings.name,
            deployed_at: e.ledger().sequence(),
        };

        let key = FactoryStorageKey::Community(index);
        e.storage().persistent().set(&key, &community);
        e.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, EXTEND_AMOUNT);

        let by_governor = FactoryStorageKey::IndexOfGovernor(governor.clone());
        e.storage().persistent().set(&by_governor, &index);
        e.storage()
            .persistent()
            .extend_ttl(&by_governor, TTL_THRESHOLD, EXTEND_AMOUNT);

        e.storage()
            .instance()
            .set(&FactoryStorageKey::Count, &(index + 1));

        CommunityDeployed {
            governor,
            founder: settings.founder,
            name: community.name.clone(),
            index,
            membership,
            weight_rule,
            timelock,
            treasury,
            registry,
        }
        .publish(e);

        community
    }

    // ################## INTERNAL ##################

    /// The address one contract of a community will be deployed at.
    ///
    /// The community's salt mixed with the contract's role, so all six are
    /// distinct and all six are predictable.
    fn address_for(e: &Env, salt: &BytesN<32>, role: &str) -> Address {
        e.deployer()
            .with_current_contract(Self::role_salt(e, salt, role))
            .deployed_address()
    }

    fn deploy_at(
        e: &Env,
        salt: &BytesN<32>,
        role: &str,
        wasm: &BytesN<32>,
        args: Vec<Val>,
    ) -> Address {
        e.deployer()
            .with_current_contract(Self::role_salt(e, salt, role))
            .deploy_v2(wasm.clone(), args)
    }

    fn role_salt(e: &Env, salt: &BytesN<32>, role: &str) -> BytesN<32> {
        let mut bytes = Bytes::from_array(e, &salt.to_array());
        bytes.extend_from_slice(role.as_bytes());
        e.crypto().sha256(&bytes).to_bytes()
    }
}
