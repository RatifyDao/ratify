//! The register's records, the settings a community is deployed with, and the
//! storage keys behind both.

use soroban_sdk::{contracterror, contractevent, contracttype, Address, BytesN, String};

/// How a community counts a vote.
///
/// Declared here rather than imported so the factory does not depend on the
/// weight rule crate to build. The variants have to match the weight rule's
/// exactly, in the same order, because that is what goes over the wire; the
/// factory's tests deploy a real weight rule and read the model back, which is
/// what keeps the two honest.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WeightModel {
    OneTokenOneVote,
    OneMemberOneVote,
    TimeWeighted,
}

/// How fast time weighted power grows, and where it stops. The same shape the
/// weight rule declares.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TenureSchedule {
    pub step_ledgers: u32,
    pub max_steps: u32,
}

/// The set of contracts that make up one community.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Community {
    /// Membership tokens and voting power.
    pub membership: Address,
    /// How a vote is priced.
    pub weight_rule: Address,
    /// Proposals and votes.
    pub governor: Address,
    /// The delay between approval and action.
    pub timelock: Address,
    /// The funds, and the policy on them.
    pub treasury: Address,
    /// The delegates' public record.
    pub registry: Address,
    /// The community's name, as its membership token carries it.
    pub name: String,
    /// The ledger the community was deployed on.
    pub deployed_at: u32,
}

/// The wasm each contract in a community is deployed from.
///
/// Fixed when the factory is deployed. A factory cannot be pointed at
/// different code later, so every community in one register runs the same
/// contracts and a member can check that by reading the hashes.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Wasms {
    pub membership: BytesN<32>,
    pub weight_rule: BytesN<32>,
    pub governor: BytesN<32>,
    pub timelock: BytesN<32>,
    pub treasury: BytesN<32>,
    pub registry: BytesN<32>,
}

/// What a community chooses when it is created.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Settings {
    /// The membership token's name, symbol and metadata location.
    pub name: String,
    pub symbol: String,
    pub base_uri: String,
    /// Who may issue membership at the start. Moved under governance later.
    pub founder: Address,
    /// Who may pull the brake on a queued action.
    pub guardian: Address,
    /// Ledgers between approval and the earliest possible action.
    pub timelock_delay: u32,
    /// Ledgers between a proposal being made and voting opening.
    pub voting_delay: u32,
    /// Ledgers voting stays open for.
    pub voting_period: u32,
    /// The voting power needed to open a proposal.
    pub proposal_threshold: u128,
    /// The share of live voting power a proposal must reach, in hundredths of
    /// a percent.
    pub quorum_bps: u32,
    /// How close a result must be to count as contested, in hundredths of a
    /// percent.
    pub contested_margin_bps: u32,
}

#[contracttype]
#[derive(Clone)]
pub enum FactoryStorageKey {
    /// The wasm hashes every community is deployed from.
    Wasms,
    /// How many communities the register holds.
    Count,
    /// A community by its position in the register.
    Community(u32),
    /// A community's position, found by its governor.
    IndexOfGovernor(Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum FactoryError {
    /// The factory has no wasm hashes.
    NotConfigured = 1,
    /// There is no community at that position.
    NotFound = 2,
    /// A community must have a delay, or the queue means nothing.
    DelayCannotBeZero = 3,
    /// A voting period of no ledgers cannot be voted in.
    VotingPeriodCannotBeZero = 4,
    /// The quorum share must be a fraction, and not zero.
    InvalidQuorum = 5,
    /// More communities than the register will hold.
    RegisterFull = 6,
}

/// Emitted when a community is deployed and entered in the register.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommunityDeployed {
    #[topic]
    pub governor: Address,
    #[topic]
    pub founder: Address,
    /// The community's name. Carried in the event so a register built from
    /// events alone can name what it holds, rather than having to go and ask
    /// each membership contract.
    pub name: String,
    pub index: u32,
    pub membership: Address,
    pub weight_rule: Address,
    pub timelock: Address,
    pub treasury: Address,
    pub registry: Address,
}
