//! Storage keys, errors and events specific to the Ratify governor, plus the
//! slice of the delegate registry it writes to.

use soroban_sdk::{
    contractclient, contracterror, contractevent, contracttype, Address, BytesN, Env, String,
};

#[contracttype]
#[derive(Clone)]
pub enum GovStorageKey {
    /// The timelock every approved action is queued into.
    Timelock,
    /// The delegate registry, written to as votes are cast.
    Registry,
    /// The share of live voting power a proposal must reach, in hundredths of
    /// a percent.
    QuorumBps,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RatifyGovernorError {
    /// The governor has not been configured.
    NotConfigured = 1,
    /// The quorum share must be a fraction, not more than the whole.
    InvalidQuorum = 2,
    /// Only the proposer may withdraw a proposal, and only before voting
    /// opens.
    NotProposer = 3,
    /// Only the guardian may stop a proposal that has already been approved.
    NotGuardian = 4,
    /// A cancellation after approval must state a reason.
    ReasonRequired = 5,
    /// The stated reason is longer than the contract will store.
    ReasonTooLong = 6,
    /// The proposal has more actions than one proposal may carry.
    TooManyActions = 7,
    /// The proposal is not in a state this action applies to.
    WrongState = 8,
}

/// Emitted when the guardian stops an approved proposal, with the reason.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalStoppedWithReason {
    #[topic]
    pub proposal_id: BytesN<32>,
    #[topic]
    pub guardian: Address,
    pub reason: String,
}

/// How an account voted, as the delegate registry records it.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Ballot {
    Against = 0,
    For = 1,
    Abstain = 2,
}

/// The part of the delegate registry the governor writes to.
#[contractclient(name = "RegistryClient")]
pub trait Registry {
    /// Tells the registry a proposal exists.
    fn open(e: &Env, proposal_id: BytesN<32>, snapshot: u32, deadline: u32);
    /// Records a vote as it is cast.
    fn record_vote(
        e: &Env,
        account: Address,
        proposal_id: BytesN<32>,
        ballot: Ballot,
        weight: u128,
    );
}

/// The part of the timelock the governor drives.
#[contractclient(name = "GuardianClient")]
pub trait GuardianOf {
    /// The address allowed to pull the brake.
    fn guardian(e: &Env) -> Address;
    /// Cancels a waiting operation, with a reason.
    fn cancel_with_reason(e: &Env, operation_id: BytesN<32>, guardian: Address, reason: String);
}
