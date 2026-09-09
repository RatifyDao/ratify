#![no_std]
//! # Ratify delegate registry
//!
//! Whether a delegate turns up, written down where anyone can read it.
//!
//! Two of the three writes come from the governor and cannot be forged.
//!
//! 1. When a proposal is created the governor tells the registry it exists,
//!    with the ledger voting power was snapshotted at and the ledger voting
//!    closes on.
//! 2. When a member votes the governor records the ballot and its weight.
//! 3. When voting has closed, **anyone** may settle an account against the
//!    proposal. Settling reads the weight rule at the snapshot. If the account
//!    held power then it was eligible, and the record gains an eligible
//!    proposal whether or not the account turned up.
//!
//! Settling is the part that makes a missed vote visible. It is permissionless
//! and its result is fixed by chain state, so nobody can settle selectively to
//! flatter or damage a delegate: the answer is the same whoever asks, and the
//! only choice anyone has is whether to bother.
//!
//! ## Contested proposals
//!
//! A proposal decided by a narrow margin is marked contested when it is first
//! settled. Turning up for the easy ones and missing the close ones is exactly
//! the behaviour a participation rate alone would hide, so the record counts
//! contested proposals separately.

mod types;

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env};

pub use crate::types::{
    AccountSettled, Ballot, Governor, GovernorClient, Proposal, ProposalOpened, Record,
    RegistryError, RegistryStorageKey, VoteCounts, VoteRecorded, WeightRule, WeightRuleClient,
};

/// Hundredths of a percent in the whole.
const BPS: u128 = 10_000;

const EXTEND_AMOUNT: u32 = 30 * 17_280;
const TTL_THRESHOLD: u32 = EXTEND_AMOUNT - 17_280;

#[contract]
pub struct RatifyDelegateRegistry;

#[contractimpl]
impl RatifyDelegateRegistry {
    /// Binds the registry to the governor whose votes it records.
    ///
    /// `contested_margin_bps` is how close a result has to be to count as
    /// contested, as a share of the votes cast. Two thousand means a proposal
    /// decided by less than twenty percent of the votes cast.
    pub fn __constructor(
        e: &Env,
        governor: Address,
        weight_rule: Address,
        contested_margin_bps: u32,
    ) {
        if contested_margin_bps as u128 > BPS {
            panic_with_error!(e, RegistryError::InvalidMargin);
        }
        e.storage().instance().set(&RegistryStorageKey::Governor, &governor);
        e.storage().instance().set(&RegistryStorageKey::WeightRule, &weight_rule);
        e.storage()
            .instance()
            .set(&RegistryStorageKey::ContestedMarginBps, &contested_margin_bps);
    }

    // ################## QUERIES ##################

    /// Returns the governor whose votes this registry records.
    pub fn governor(e: &Env) -> Address {
        e.storage()
            .instance()
            .get(&RegistryStorageKey::Governor)
            .unwrap_or_else(|| panic_with_error!(e, RegistryError::NotConfigured))
    }

    /// Returns the weight rule the registry reads to decide eligibility.
    pub fn weight_rule(e: &Env) -> Address {
        e.storage()
            .instance()
            .get(&RegistryStorageKey::WeightRule)
            .unwrap_or_else(|| panic_with_error!(e, RegistryError::NotConfigured))
    }

    /// Returns the margin below which a proposal counts as contested.
    pub fn contested_margin_bps(e: &Env) -> u32 {
        e.storage().instance().get(&RegistryStorageKey::ContestedMarginBps).unwrap_or(0)
    }

    /// Returns an account's public record.
    ///
    /// An account nobody has ever settled reads as all zeros, which is
    /// honest: there is nothing known about them yet.
    pub fn record(e: &Env, account: Address) -> Record {
        e.storage()
            .persistent()
            .get(&RegistryStorageKey::Record(account))
            .unwrap_or_default()
    }

    /// Returns an account's participation rate, in hundredths of a percent.
    ///
    /// Returns `None` when the account has never been eligible for anything,
    /// because a rate out of nothing is not zero, it is unknown. The delegates
    /// page prints that distinction rather than showing a new delegate as
    /// nought percent.
    pub fn participation_bps(e: &Env, account: Address) -> Option<u32> {
        let record = Self::record(e, account);
        if record.eligible == 0 {
            return None;
        }
        Some(((record.voted as u128 * BPS) / record.eligible as u128) as u32)
    }

    /// Returns an account's turnout on contested proposals, in hundredths of a
    /// percent.
    ///
    /// The figure that separates a delegate who shows up for the close ones
    /// from one who shows up for the easy ones.
    pub fn contested_participation_bps(e: &Env, account: Address) -> Option<u32> {
        let record = Self::record(e, account);
        if record.contested_eligible == 0 {
            return None;
        }
        Some(((record.contested_voted as u128 * BPS) / record.contested_eligible as u128) as u32)
    }

    /// Returns a proposal as the registry knows it.
    pub fn proposal(e: &Env, proposal_id: BytesN<32>) -> Option<Proposal> {
        e.storage().persistent().get(&RegistryStorageKey::Proposal(proposal_id))
    }

    /// Returns how an account voted on a proposal, if they did.
    pub fn ballot(e: &Env, proposal_id: BytesN<32>, account: Address) -> Option<Ballot> {
        e.storage().persistent().get(&RegistryStorageKey::Ballot(proposal_id, account))
    }

    /// Returns whether an account has been settled against a proposal.
    pub fn is_settled(e: &Env, proposal_id: BytesN<32>, account: Address) -> bool {
        e.storage()
            .persistent()
            .get(&RegistryStorageKey::Settled(proposal_id, account))
            .unwrap_or(false)
    }

    // ################## WRITTEN BY THE GOVERNOR ##################

    /// Records that a proposal exists. Only the governor may call this.
    pub fn open(e: &Env, proposal_id: BytesN<32>, snapshot: u32, deadline: u32) {
        Self::require_governor(e);
        if Self::proposal(e, proposal_id.clone()).is_some() {
            panic_with_error!(e, RegistryError::ProposalAlreadyOpen);
        }
        let proposal = Proposal {
            snapshot,
            deadline,
            for_votes: 0,
            against_votes: 0,
            contested: false,
            tallied: false,
        };
        Self::put_proposal(e, &proposal_id, &proposal);
        ProposalOpened { proposal_id, snapshot, deadline }.publish(e);
    }

    /// Records a vote. Only the governor may call this.
    ///
    /// Written at the moment the vote is cast, from inside the governor, so a
    /// delegate cannot decide afterwards whether their vote should count
    /// towards their record.
    pub fn record_vote(
        e: &Env,
        account: Address,
        proposal_id: BytesN<32>,
        ballot: Ballot,
        weight: u128,
    ) {
        Self::require_governor(e);
        if Self::proposal(e, proposal_id.clone()).is_none() {
            panic_with_error!(e, RegistryError::ProposalNotFound);
        }

        let key = RegistryStorageKey::Ballot(proposal_id.clone(), account.clone());
        e.storage().persistent().set(&key, &ballot);
        e.storage().persistent().extend_ttl(&key, TTL_THRESHOLD, EXTEND_AMOUNT);

        VoteRecorded { account, proposal_id, ballot, weight }.publish(e);
    }

    // ################## SETTLED BY ANYONE ##################

    /// Settles an account against a closed proposal.
    ///
    /// Anyone may call this, for anyone, once voting has closed. It reads the
    /// weight rule at the proposal's snapshot: an account that held power then
    /// was eligible, and the record gains an eligible proposal whether or not
    /// they turned up. That is the only way a missed vote can be counted, and
    /// it is why it cannot be avoided.
    ///
    /// Settling twice does nothing, so there is no way to inflate a record by
    /// calling repeatedly.
    pub fn settle(e: &Env, account: Address, proposal_id: BytesN<32>) {
        let mut proposal = Self::proposal(e, proposal_id.clone())
            .unwrap_or_else(|| panic_with_error!(e, RegistryError::ProposalNotFound));

        if e.ledger().sequence() <= proposal.deadline {
            panic_with_error!(e, RegistryError::VotingStillOpen);
        }
        if Self::is_settled(e, proposal_id.clone(), account.clone()) {
            panic_with_error!(e, RegistryError::AlreadySettled);
        }

        let weight = WeightRuleClient::new(e, &Self::weight_rule(e))
            .weight_at(&account, &proposal.snapshot);
        if weight == 0 {
            panic_with_error!(e, RegistryError::NotEligible);
        }

        if !proposal.tallied {
            proposal = Self::tally(e, &proposal_id, proposal);
        }

        let ballot = Self::ballot(e, proposal_id.clone(), account.clone());
        let turned_up = ballot.is_some();
        let with_outcome = match ballot {
            Some(Ballot::For) => proposal.for_votes > proposal.against_votes,
            Some(Ballot::Against) => proposal.against_votes >= proposal.for_votes,
            _ => false,
        };

        let mut record = Self::record(e, account.clone());
        record.eligible += 1;
        if turned_up {
            record.voted += 1;
        }
        if proposal.contested {
            record.contested_eligible += 1;
            if turned_up {
                record.contested_voted += 1;
            }
            if with_outcome {
                record.contested_with_outcome += 1;
            }
        }
        if proposal.deadline > record.last_settled_deadline {
            record.last_settled_deadline = proposal.deadline;
        }

        let record_key = RegistryStorageKey::Record(account.clone());
        e.storage().persistent().set(&record_key, &record);
        e.storage().persistent().extend_ttl(&record_key, TTL_THRESHOLD, EXTEND_AMOUNT);

        let settled_key = RegistryStorageKey::Settled(proposal_id.clone(), account.clone());
        e.storage().persistent().set(&settled_key, &true);
        e.storage().persistent().extend_ttl(&settled_key, TTL_THRESHOLD, EXTEND_AMOUNT);

        AccountSettled {
            account,
            proposal_id,
            turned_up,
            contested: proposal.contested,
            with_outcome,
        }
        .publish(e);
    }

    // ################## INTERNAL ##################

    fn require_governor(e: &Env) {
        Self::governor(e).require_auth();
    }

    /// Reads the final tallies from the governor and decides whether the
    /// proposal was contested.
    ///
    /// Done once, on the first settlement after voting closes, so every
    /// account settled against the proposal is judged against the same
    /// numbers.
    fn tally(e: &Env, proposal_id: &BytesN<32>, mut proposal: Proposal) -> Proposal {
        let counts = GovernorClient::new(e, &Self::governor(e))
            .get_proposal_vote_counts(proposal_id);

        let decisive = counts.for_votes + counts.against_votes;
        let margin = counts.for_votes.abs_diff(counts.against_votes);
        let threshold = Self::contested_margin_bps(e) as u128;

        proposal.for_votes = counts.for_votes;
        proposal.against_votes = counts.against_votes;
        proposal.contested = decisive > 0 && margin * BPS < decisive * threshold;
        proposal.tallied = true;

        Self::put_proposal(e, proposal_id, &proposal);
        proposal
    }

    fn put_proposal(e: &Env, proposal_id: &BytesN<32>, proposal: &Proposal) {
        let key = RegistryStorageKey::Proposal(proposal_id.clone());
        e.storage().persistent().set(&key, proposal);
        e.storage().persistent().extend_ttl(&key, TTL_THRESHOLD, EXTEND_AMOUNT);
    }
}
