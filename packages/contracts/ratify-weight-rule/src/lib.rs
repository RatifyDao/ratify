#![no_std]
//! # Ratify weight rule
//!
//! One question, one contract: how much is this account's vote worth at this
//! point in time.
//!
//! A community picks its model when it deploys. Three models at launch.
//!
//! **One token one vote.** The familiar model. Weight is the membership
//! tokens whose live grant answers to you.
//!
//! **One member one vote.** Holding ten membership tokens still gives you one
//! vote. A delegate's weight is the number of members who granted to them,
//! not the tokens those members hold.
//!
//! **Time weighted.** Weight grows in steps with how long the account has
//! been a member without a break, up to a ceiling the community sets.
//!
//! ## How the governor reads this
//!
//! The contract answers on the standard votes interface, which is what the
//! OpenZeppelin governor calls to price a vote and to check a proposer is
//! above the threshold. That is the whole integration: the governor is
//! pointed at this contract instead of at the membership token, and neither
//! of them needs to know which model is in force.

mod types;

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, panic_with_error, Address, Env};
use stellar_governance::votes::Votes;

pub use crate::types::{
    Membership, MembershipClient, TenureSchedule, WeightModel, WeightRuleError,
    WeightRuleStorageKey,
};

/// The number of steps of tenure a time weighted community may grant, at most.
///
/// A ceiling has to exist. Without one, the earliest members eventually
/// outvote everyone who came after them, which is a different kind of capture
/// from the one this model exists to prevent.
pub const MAX_TENURE_STEPS: u32 = 20;

#[contract]
pub struct RatifyWeightRule;

#[contractimpl]
impl RatifyWeightRule {
    /// Fixes the community's voting model.
    ///
    /// The model cannot be changed afterwards. Changing how votes are counted
    /// mid-life is the kind of decision that should show up as a new set of
    /// contracts a member can see, not as a setting somebody flipped.
    pub fn __constructor(
        e: &Env,
        membership: Address,
        model: WeightModel,
        tenure: Option<TenureSchedule>,
    ) {
        if model == WeightModel::TimeWeighted {
            let schedule = tenure
                .clone()
                .unwrap_or_else(|| panic_with_error!(e, WeightRuleError::InvalidModel));
            if schedule.step_ledgers == 0
                || schedule.max_steps == 0
                || schedule.max_steps > MAX_TENURE_STEPS
            {
                panic_with_error!(e, WeightRuleError::InvalidModel);
            }
            e.storage()
                .instance()
                .set(&WeightRuleStorageKey::Tenure, &schedule);
        }
        e.storage()
            .instance()
            .set(&WeightRuleStorageKey::Membership, &membership);
        e.storage()
            .instance()
            .set(&WeightRuleStorageKey::Model, &model);
    }

    /// Returns the tenure schedule, if the community is time weighted.
    pub fn tenure_schedule(e: &Env) -> Option<TenureSchedule> {
        e.storage().instance().get(&WeightRuleStorageKey::Tenure)
    }

    /// Returns the membership contract this rule reads.
    pub fn membership(e: &Env) -> Address {
        e.storage()
            .instance()
            .get(&WeightRuleStorageKey::Membership)
            .unwrap_or_else(|| panic_with_error!(e, WeightRuleError::NotConfigured))
    }

    /// Returns the model in force.
    pub fn model(e: &Env) -> WeightModel {
        e.storage()
            .instance()
            .get(&WeightRuleStorageKey::Model)
            .unwrap_or_else(|| panic_with_error!(e, WeightRuleError::NotConfigured))
    }

    /// Returns the weight of an account's vote at a ledger.
    ///
    /// The same answer [`Votes::get_votes_at_checkpoint`] gives, under a name
    /// that says what it is. The interface page and the proposal page both
    /// read this so a member can see their own weight before voting.
    pub fn weight_at(e: &Env, account: Address, ledger: u32) -> u128 {
        let membership = MembershipClient::new(e, &Self::membership(e));

        match Self::model(e) {
            WeightModel::OneTokenOneVote => membership.votes_at(&account, &ledger),

            WeightModel::OneMemberOneVote => membership.heads_at(&account, &ledger),

            WeightModel::TimeWeighted => {
                let base = membership.votes_at(&account, &ledger);
                if base == 0 {
                    return 0;
                }
                let schedule = Self::tenure_schedule(e)
                    .unwrap_or_else(|| panic_with_error!(e, WeightRuleError::NotConfigured));
                let steps = (membership.tenure_at(&account, &ledger) / schedule.step_ledgers)
                    .min(schedule.max_steps);
                base * (1 + steps as u128)
            }
        }
    }

    /// Returns the community's total live weight at a ledger.
    ///
    /// This is the denominator a quorum is measured against, so it has to be
    /// counted the same way as the numerator. Under one member one vote that
    /// is a head count, and under the other two it is voting power.
    ///
    /// Time weighted is the one case where the two cannot agree exactly. Its
    /// total is the live power without any tenure multiplier applied, because
    /// applying one would require walking every member. A community running
    /// time weighting should read its quorum as a share of unweighted power,
    /// and the interface says so on the page.
    pub fn total_weight_at(e: &Env, ledger: u32) -> u128 {
        let membership = MembershipClient::new(e, &Self::membership(e));

        match Self::model(e) {
            WeightModel::OneMemberOneVote => membership.live_heads_at(&ledger),
            _ => membership.live_total_at(&ledger),
        }
    }
}

#[contractimpl(contracttrait)]
impl Votes for RatifyWeightRule {
    /// Returns the weight of an account's vote as it stands now.
    fn get_votes(e: &Env, account: Address) -> u128 {
        Self::weight_at(e, account, e.ledger().sequence())
    }

    /// Returns the weight of an account's vote at a past ledger.
    ///
    /// This is the method the governor calls when a member votes and when a
    /// proposal is created, so it is the one that decides outcomes.
    fn get_votes_at_checkpoint(e: &Env, account: Address, ledger: u32) -> u128 {
        Self::weight_at(e, account, ledger)
    }

    /// Returns the community's total live weight.
    fn get_total_supply(e: &Env) -> u128 {
        Self::total_weight_at(e, e.ledger().sequence())
    }

    /// Returns the community's total live weight at a past ledger.
    fn get_total_supply_at_checkpoint(e: &Env, ledger: u32) -> u128 {
        Self::total_weight_at(e, ledger)
    }

    /// Returns who an account's power answers to.
    ///
    /// Read through from the membership contract, which is where grants live.
    fn get_delegate(e: &Env, account: Address) -> Option<Address> {
        MembershipClient::new(e, &Self::membership(e))
            .grant(&account)
            .map(|grant| grant.delegatee)
    }

    /// Delegates voting power. Always fails.
    ///
    /// A grant in RatifyDAO carries a term, so it is made on the membership
    /// contract where the term lives. A delegation made here would have no
    /// expiry, which is the thing the product exists to fix.
    fn delegate(e: &Env, _account: Address, _delegatee: Address) {
        panic_with_error!(e, WeightRuleError::DelegateOnMembership);
    }
}
