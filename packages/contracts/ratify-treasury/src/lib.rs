#![no_std]
//! # Ratify treasury
//!
//! A per-community treasury that holds funds and pays out only on instruction
//! from the timelock, inside a policy that governance itself sets.
//!
//! The contract is the last check in the permission chain. The governor can
//! queue an action but cannot spend. The timelock can call this contract, but
//! only after its delay has passed. This contract then refuses any instruction
//! that breaches policy, whatever the vote said.
//!
//! Policy covers four things:
//!
//! * which assets may leave, expressed as the set of assets that have a policy
//! * the largest amount a single payment may move
//! * the largest total that may leave inside a rolling window of ledgers
//! * an optional allowlist of destinations
//!
//! Changing any of it is itself a call from the timelock, so a policy change
//! is a proposal and waits out the same delay as a payment.
//!
//! No deployer key, operator key or frontend appears anywhere in this
//! contract. The only privileged address is the timelock, and it is set once
//! at deployment.

mod types;

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, panic_with_error, token, Address, BytesN, Env, Vec};

pub use crate::types::{
    AssetPolicy, DepositMade, DestinationRestrictionSet, DestinationSet, Outflow, PaymentMade,
    PolicyRemoved, PolicySet, TreasuryError, TreasuryStorageKey,
};

/// Ledgers of contract lifetime restored on each write, and the threshold
/// below which a restore happens. Thirty days, in five second ledgers.
const EXTEND_AMOUNT: u32 = 30 * 17_280;
const TTL_THRESHOLD: u32 = EXTEND_AMOUNT - 17_280;

/// The most payments an asset may have inside one rolling window.
///
/// The window is measured by walking the payments still inside it, so it has
/// to be bounded. A community that hits this is making hundreds of payments
/// from one asset inside its own window and should widen the window rather
/// than have the treasury silently forget the earlier ones.
const MAX_WINDOW_ENTRIES: u32 = 100;

#[contract]
pub struct RatifyTreasury;

#[contractimpl]
impl RatifyTreasury {
    /// Binds the treasury to its timelock.
    ///
    /// The timelock is the only address this contract will take instructions
    /// from, and it cannot be changed afterwards. A community that wants a
    /// different timelock deploys a new treasury and moves the funds by
    /// proposal, which is visible to every member.
    pub fn __constructor(e: &Env, timelock: Address) {
        e.storage()
            .instance()
            .set(&TreasuryStorageKey::Timelock, &timelock);
        e.storage()
            .instance()
            .set(&TreasuryStorageKey::RestrictDestinations, &false);
        e.storage()
            .instance()
            .set(&TreasuryStorageKey::OutflowCount, &0u32);
    }

    // ################## QUERIES ##################

    /// Returns the timelock this treasury obeys.
    pub fn timelock(e: &Env) -> Address {
        e.storage()
            .instance()
            .get(&TreasuryStorageKey::Timelock)
            .unwrap_or_else(|| panic_with_error!(e, TreasuryError::TimelockNotSet))
    }

    /// Returns the treasury's balance of an asset.
    pub fn balance(e: &Env, asset: Address) -> i128 {
        token::TokenClient::new(e, &asset).balance(&e.current_contract_address())
    }

    /// Returns the policy in force for an asset, or `None` if the asset may
    /// not leave the treasury.
    pub fn policy(e: &Env, asset: Address) -> Option<AssetPolicy> {
        e.storage()
            .persistent()
            .get(&TreasuryStorageKey::Policy(asset))
    }

    /// Returns whether payments are restricted to the destination allowlist.
    pub fn destinations_restricted(e: &Env) -> bool {
        e.storage()
            .instance()
            .get(&TreasuryStorageKey::RestrictDestinations)
            .unwrap_or(false)
    }

    /// Returns whether a destination may receive funds under the policy as it
    /// currently stands.
    pub fn destination_allowed(e: &Env, destination: Address) -> bool {
        if !Self::destinations_restricted(e) {
            return true;
        }
        e.storage()
            .persistent()
            .get(&TreasuryStorageKey::Destination(destination))
            .unwrap_or(false)
    }

    /// Returns the total already paid out of an asset inside the current
    /// rolling window.
    pub fn window_spent(e: &Env, asset: Address) -> i128 {
        match Self::policy(e, asset.clone()) {
            Some(policy) => {
                let entries = Self::window_entries(e, &asset);
                Self::sum_window(e, &entries, policy.window_ledgers)
            }
            None => 0,
        }
    }

    /// Returns how much of an asset may still leave inside the current window.
    ///
    /// This is the number the new proposal form checks against, so a member
    /// cannot submit a proposal that could never execute.
    pub fn window_headroom(e: &Env, asset: Address) -> i128 {
        match Self::policy(e, asset.clone()) {
            Some(policy) => {
                let spent = Self::window_spent(e, asset);
                if spent >= policy.window_cap {
                    0
                } else {
                    policy.window_cap - spent
                }
            }
            None => 0,
        }
    }

    /// Returns the number of payments the treasury has ever made.
    pub fn outflow_count(e: &Env) -> u32 {
        e.storage()
            .instance()
            .get(&TreasuryStorageKey::OutflowCount)
            .unwrap_or(0)
    }

    /// Returns a single payment by index, oldest first.
    pub fn outflow(e: &Env, index: u32) -> Outflow {
        e.storage()
            .persistent()
            .get(&TreasuryStorageKey::Outflow(index))
            .unwrap_or_else(|| panic_with_error!(e, TreasuryError::OutflowNotFound))
    }

    /// Reports whether a payment would be accepted right now, without making
    /// it.
    ///
    /// The proposal form and the simulated preview on a proposal page both
    /// read this, so a member sees before voting whether the proposal comes
    /// close to a limit or breaches one.
    pub fn would_allow(e: &Env, asset: Address, to: Address, amount: i128) -> bool {
        if amount <= 0 {
            return false;
        }
        let policy = match Self::policy(e, asset.clone()) {
            Some(policy) => policy,
            None => return false,
        };
        if amount > policy.per_payment_cap {
            return false;
        }
        if !Self::destination_allowed(e, to) {
            return false;
        }
        let entries = Self::window_entries(e, &asset);
        if entries.len() >= MAX_WINDOW_ENTRIES {
            return false;
        }
        Self::sum_window(e, &entries, policy.window_ledgers) + amount <= policy.window_cap
    }

    // ################## FUNDING ##################

    /// Moves funds into the treasury and records that it happened.
    ///
    /// Anyone may fund a treasury. A plain transfer to the contract address
    /// also works and is counted in the balance, but it produces no event, so
    /// funding through this method is what the history page can show.
    pub fn deposit(e: &Env, from: Address, asset: Address, amount: i128) {
        from.require_auth();
        if amount <= 0 {
            panic_with_error!(e, TreasuryError::InvalidAmount);
        }
        token::TokenClient::new(e, &asset).transfer(&from, e.current_contract_address(), &amount);
        DepositMade {
            asset,
            from,
            amount,
        }
        .publish(e);
    }

    // ################## SPENDING ##################

    /// Pays out of the treasury.
    ///
    /// Only the timelock may call this, and only after its delay has passed.
    /// Every policy limit is checked here rather than at the vote, so a
    /// proposal that breaches one cannot execute no matter how it was voted
    /// on.
    ///
    /// # Errors
    ///
    /// * [`TreasuryError::NotTimelock`] - the caller is not the timelock.
    /// * [`TreasuryError::InvalidAmount`] - the amount is zero or negative.
    /// * [`TreasuryError::AssetNotAllowed`] - the asset has no policy.
    /// * [`TreasuryError::OverPerPaymentCap`] - over the per-payment cap.
    /// * [`TreasuryError::DestinationNotAllowed`] - destinations are
    ///   restricted and this one is not allowed.
    /// * [`TreasuryError::OverWindowCap`] - over the rolling window cap.
    /// * [`TreasuryError::WindowFull`] - too many payments inside the window.
    pub fn pay(e: &Env, asset: Address, to: Address, amount: i128, proposal_id: BytesN<32>) -> u32 {
        Self::require_timelock(e);

        if amount <= 0 {
            panic_with_error!(e, TreasuryError::InvalidAmount);
        }

        let policy = Self::policy(e, asset.clone())
            .unwrap_or_else(|| panic_with_error!(e, TreasuryError::AssetNotAllowed));

        if amount > policy.per_payment_cap {
            panic_with_error!(e, TreasuryError::OverPerPaymentCap);
        }

        if !Self::destination_allowed(e, to.clone()) {
            panic_with_error!(e, TreasuryError::DestinationNotAllowed);
        }

        let ledger = e.ledger().sequence();
        let kept = Self::prune_window(e, &asset, policy.window_ledgers, ledger);
        let spent: i128 = kept.iter().map(|(_, amount)| amount).sum();

        if spent + amount > policy.window_cap {
            panic_with_error!(e, TreasuryError::OverWindowCap);
        }
        if kept.len() >= MAX_WINDOW_ENTRIES {
            panic_with_error!(e, TreasuryError::WindowFull);
        }

        let mut window = kept;
        window.push_back((ledger, amount));
        e.storage()
            .persistent()
            .set(&TreasuryStorageKey::Window(asset.clone()), &window);
        Self::extend(e, &TreasuryStorageKey::Window(asset.clone()));

        token::TokenClient::new(e, &asset).transfer(&e.current_contract_address(), &to, &amount);

        let index = Self::outflow_count(e);
        let outflow = Outflow {
            asset: asset.clone(),
            to: to.clone(),
            amount,
            ledger,
            proposal_id: proposal_id.clone(),
        };
        e.storage()
            .persistent()
            .set(&TreasuryStorageKey::Outflow(index), &outflow);
        Self::extend(e, &TreasuryStorageKey::Outflow(index));
        e.storage()
            .instance()
            .set(&TreasuryStorageKey::OutflowCount, &(index + 1));

        PaymentMade {
            asset,
            to,
            proposal_id,
            amount,
            index,
        }
        .publish(e);

        index
    }

    // ################## POLICY ##################

    /// Sets the spending policy for an asset, which is also what admits the
    /// asset to the allowlist.
    ///
    /// Only the timelock may call this, so a policy change is a proposal and
    /// waits out the same delay as a payment.
    pub fn set_policy(
        e: &Env,
        asset: Address,
        per_payment_cap: i128,
        window_cap: i128,
        window_ledgers: u32,
    ) {
        Self::require_timelock(e);
        if per_payment_cap <= 0 || window_cap <= 0 || window_ledgers == 0 {
            panic_with_error!(e, TreasuryError::InvalidPolicy);
        }
        let policy = AssetPolicy {
            per_payment_cap,
            window_cap,
            window_ledgers,
        };
        e.storage()
            .persistent()
            .set(&TreasuryStorageKey::Policy(asset.clone()), &policy);
        Self::extend(e, &TreasuryStorageKey::Policy(asset.clone()));
        PolicySet {
            asset,
            per_payment_cap,
            window_cap,
            window_ledgers,
        }
        .publish(e);
    }

    /// Withdraws an asset's policy, after which it cannot leave the treasury.
    pub fn remove_policy(e: &Env, asset: Address) {
        Self::require_timelock(e);
        e.storage()
            .persistent()
            .remove(&TreasuryStorageKey::Policy(asset.clone()));
        PolicyRemoved { asset }.publish(e);
    }

    /// Turns the destination allowlist on or off.
    ///
    /// With it off, any address may receive funds, subject to the amount
    /// limits. With it on, only allowed destinations may.
    pub fn set_destination_restriction(e: &Env, restricted: bool) {
        Self::require_timelock(e);
        e.storage()
            .instance()
            .set(&TreasuryStorageKey::RestrictDestinations, &restricted);
        DestinationRestrictionSet { restricted }.publish(e);
    }

    /// Adds a destination to the allowlist or removes it.
    pub fn set_destination(e: &Env, destination: Address, allowed: bool) {
        Self::require_timelock(e);
        if allowed {
            e.storage()
                .persistent()
                .set(&TreasuryStorageKey::Destination(destination.clone()), &true);
            Self::extend(e, &TreasuryStorageKey::Destination(destination.clone()));
        } else {
            e.storage()
                .persistent()
                .remove(&TreasuryStorageKey::Destination(destination.clone()));
        }
        DestinationSet {
            destination,
            allowed,
        }
        .publish(e);
    }

    // ################## INTERNAL ##################

    /// Requires that the caller is the timelock.
    ///
    /// This is the whole of the treasury's access control. There is no admin,
    /// no owner and no pause.
    fn require_timelock(e: &Env) {
        let timelock = Self::timelock(e);
        timelock.require_auth();
    }

    /// Returns the payments recorded against an asset, without pruning.
    fn window_entries(e: &Env, asset: &Address) -> Vec<(u32, i128)> {
        e.storage()
            .persistent()
            .get(&TreasuryStorageKey::Window(asset.clone()))
            .unwrap_or_else(|| Vec::new(e))
    }

    /// Sums the entries that are still inside the window.
    fn sum_window(e: &Env, entries: &Vec<(u32, i128)>, window_ledgers: u32) -> i128 {
        let now = e.ledger().sequence();
        let cutoff = now.saturating_sub(window_ledgers);
        let mut total: i128 = 0;
        for (ledger, amount) in entries.iter() {
            if ledger > cutoff {
                total += amount;
            }
        }
        total
    }

    /// Drops the entries that have aged out of the window and returns the rest.
    fn prune_window(e: &Env, asset: &Address, window_ledgers: u32, now: u32) -> Vec<(u32, i128)> {
        let cutoff = now.saturating_sub(window_ledgers);
        let mut kept: Vec<(u32, i128)> = Vec::new(e);
        for (ledger, amount) in Self::window_entries(e, asset).iter() {
            if ledger > cutoff {
                kept.push_back((ledger, amount));
            }
        }
        kept
    }

    /// Restores the lifetime of a persistent entry that was just written.
    fn extend(e: &Env, key: &TreasuryStorageKey) {
        e.storage()
            .persistent()
            .extend_ttl(key, TTL_THRESHOLD, EXTEND_AMOUNT);
    }
}
