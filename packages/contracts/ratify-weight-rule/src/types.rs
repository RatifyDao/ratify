//! The models, the storage keys, and the slice of the membership contract
//! this rule reads.

use soroban_sdk::{contractclient, contracterror, contracttype, Address, Env};

/// How a community counts a vote.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WeightModel {
    /// Weight is the membership tokens whose live grant answers to you.
    OneTokenOneVote,
    /// Weight is the number of members whose live grant answers to you.
    /// Holding ten tokens still gives one vote.
    OneMemberOneVote,
    /// Weight grows in steps with unbroken membership, to a ceiling set by
    /// the community's [`TenureSchedule`].
    TimeWeighted,
}

/// How fast time weighted power grows, and where it stops.
///
/// An account gains one extra multiple of its power for every `step_ledgers`
/// it has been a member, up to `max_steps` of them. A member of one step's
/// standing votes at twice their power, two steps at three times, and so on.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TenureSchedule {
    pub step_ledgers: u32,
    pub max_steps: u32,
}

#[contracttype]
#[derive(Clone)]
pub enum WeightRuleStorageKey {
    /// The membership contract this rule reads.
    Membership,
    /// The model in force.
    Model,
    /// The tenure schedule, present only under time weighting.
    Tenure,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum WeightRuleError {
    /// The rule has no membership contract or no model.
    NotConfigured = 1,
    /// The model's parameters are missing or cannot be used.
    InvalidModel = 2,
    /// Grants are made on the membership contract, where terms live.
    DelegateOnMembership = 3,
}

/// A grant of voting power as the membership contract records it.
///
/// Declared here rather than imported so the weight rule does not depend on
/// the membership crate to build. The shape has to match; the membership
/// contract's tests and this contract's tests both exercise the pair.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Grant {
    pub delegatee: Address,
    pub units: u128,
    pub expires_at: u32,
    pub granted_at: u32,
}

/// The part of the membership contract a weight rule needs.
#[contractclient(name = "MembershipClient")]
pub trait Membership {
    /// Voting power answering to an account at a ledger.
    fn votes_at(e: &Env, account: Address, ledger: u32) -> u128;
    /// Members whose grant answered to an account at a ledger.
    fn heads_at(e: &Env, account: Address, ledger: u32) -> u128;
    /// The community's live voting power at a ledger.
    fn live_total_at(e: &Env, ledger: u32) -> u128;
    /// The community's live member count at a ledger.
    fn live_heads_at(e: &Env, ledger: u32) -> u128;
    /// Ledgers of unbroken membership, measured at a ledger.
    fn tenure_at(e: &Env, account: Address, ledger: u32) -> u32;
    /// The grant an account has made, if any.
    fn grant(e: &Env, account: Address) -> Option<Grant>;
}
