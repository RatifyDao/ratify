//! The record itself, plus the slices of the governor and the weight rule the
//! registry reads when it settles a proposal.

use soroban_sdk::{contractclient, contracterror, contractevent, contracttype, Address, BytesN, Env};

/// A delegate's public record.
///
/// Every figure here is produced by the contracts themselves. The delegate
/// cannot edit it and neither can the platform.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct Record {
    /// Proposals the account held power on and could have voted on.
    pub eligible: u32,
    /// Proposals the account actually voted on.
    pub voted: u32,
    /// Of the eligible proposals, those that were decided by a narrow margin.
    pub contested_eligible: u32,
    /// Of those, the ones the account voted on at all.
    pub contested_voted: u32,
    /// Of those, the ones where the account's vote matched the outcome.
    pub contested_with_outcome: u32,
    /// The last proposal settled against this account, so the interface can
    /// show how current the record is.
    pub last_settled_deadline: u32,
}

/// A proposal as the registry knows it.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    /// The ledger voting power was snapshotted at.
    pub snapshot: u32,
    /// The last ledger a vote could be cast on.
    pub deadline: u32,
    /// Votes for, once the proposal has been settled at least once.
    pub for_votes: u128,
    /// Votes against, once settled.
    pub against_votes: u128,
    /// Whether the margin was narrow enough to call the proposal contested.
    pub contested: bool,
    /// Whether the tallies above have been read from the governor yet.
    pub tallied: bool,
}

/// How an account voted on a proposal.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Ballot {
    Against = 0,
    For = 1,
    Abstain = 2,
}

#[contracttype]
#[derive(Clone)]
pub enum RegistryStorageKey {
    /// The governor whose votes this registry records.
    Governor,
    /// The weight rule, read to decide who was eligible.
    WeightRule,
    /// The margin, in hundredths of a percent, below which a proposal counts
    /// as contested.
    ContestedMarginBps,
    /// A proposal the governor has opened.
    Proposal(BytesN<32>),
    /// How an account voted on a proposal.
    Ballot(BytesN<32>, Address),
    /// Whether an account has been settled against a proposal.
    Settled(BytesN<32>, Address),
    /// An account's public record.
    Record(Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RegistryError {
    /// The registry has not been configured.
    NotConfigured = 1,
    /// Only the governor may write to the record.
    NotGovernor = 2,
    /// The proposal is not known to the registry.
    ProposalNotFound = 3,
    /// The proposal is already recorded.
    ProposalAlreadyOpen = 4,
    /// Voting on the proposal has not closed, so nothing can be settled yet.
    VotingStillOpen = 5,
    /// The account has already been settled against this proposal.
    AlreadySettled = 6,
    /// The account held no power at the snapshot, so it was never eligible.
    NotEligible = 7,
    /// The margin must be a fraction, not more than the whole.
    InvalidMargin = 8,
}

/// Emitted when the governor tells the registry a proposal exists.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalOpened {
    #[topic]
    pub proposal_id: BytesN<32>,
    pub snapshot: u32,
    pub deadline: u32,
}

/// Emitted when the governor records a vote.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoteRecorded {
    #[topic]
    pub account: Address,
    #[topic]
    pub proposal_id: BytesN<32>,
    pub ballot: Ballot,
    pub weight: u128,
}

/// Emitted when an account is settled against a closed proposal, which is the
/// moment a missed vote becomes part of the record.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountSettled {
    #[topic]
    pub account: Address,
    #[topic]
    pub proposal_id: BytesN<32>,
    pub turned_up: bool,
    pub contested: bool,
    pub with_outcome: bool,
}

/// The part of the governor the registry reads.
#[contractclient(name = "GovernorClient")]
pub trait Governor {
    /// The vote tallies for a proposal.
    fn get_proposal_vote_counts(e: &Env, proposal_id: BytesN<32>) -> VoteCounts;
}

/// Vote tallies as the governor reports them.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoteCounts {
    pub against_votes: u128,
    pub for_votes: u128,
    pub abstain_votes: u128,
}

/// The part of the weight rule the registry reads.
#[contractclient(name = "WeightRuleClient")]
pub trait WeightRule {
    /// The weight an account's vote carried at a ledger.
    fn weight_at(e: &Env, account: Address, ledger: u32) -> u128;
}
