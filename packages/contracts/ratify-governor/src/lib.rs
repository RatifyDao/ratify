#![no_std]
//! # Ratify governor
//!
//! Proposals, votes, and the decision to queue.
//!
//! Three things separate this from the governor it is modelled on.
//!
//! **Queuing is on.** A succeeded proposal must be queued into the timelock,
//! and the timelock will not run it until the delay has passed.
//!
//! **Quorum is measured against live voting power.** Not against every token
//! ever issued, and not against a fixed number somebody set once. The
//! denominator is the power that had not lapsed at the proposal's snapshot,
//! which is what makes a real quorum reachable instead of a decorative one.
//!
//! **The governor cannot spend.** It can put an action into the queue and
//! nothing else. It holds no funds, and the treasury does not take its calls.

mod types;

#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contractimpl, panic_with_error, Address, Bytes, BytesN, Env, String, Symbol, Val, Vec,
};
use stellar_governance::{
    governor::{
        storage::{GovernorStorageKey, ProposalCore},
        emit_proposal_cancelled, emit_proposal_executed, hash_proposal, Governor, ProposalState,
    },
    timelock::{hash_operation, Operation, TimelockClient},
    votes::VotesClient,
};

pub use crate::types::{
    RatifyGovernorError, Ballot, GovStorageKey, GuardianClient, GuardianOf, ProposalStoppedWithReason,
    Registry, RegistryClient,
};

/// Hundredths of a percent in the whole.
const BPS: u128 = 10_000;

/// The most actions one proposal may carry.
///
/// A proposal a member cannot hold in their head is a proposal they are
/// approving on trust. The first release restricts proposals to treasury
/// payments and governance parameter changes, and none of those need more
/// than a handful of calls.
pub const MAX_ACTIONS: u32 = 8;

/// The longest a cancellation reason may be, in bytes.
const MAX_REASON_LENGTH: u32 = 512;

#[contract]
pub struct RatifyGovernor;

#[contractimpl]
impl RatifyGovernor {
    /// Wires the governor to the rest of the community's contracts.
    ///
    /// `weight_rule` is the contract that prices a vote, and it stands where
    /// a plain membership token would in a simpler design. That indirection
    /// is what lets a community choose one token one vote, one member one
    /// vote or time weighting without a different governor.
    #[allow(clippy::too_many_arguments)]
    pub fn __constructor(
        e: &Env,
        weight_rule: Address,
        timelock: Address,
        registry: Option<Address>,
        name: String,
        voting_delay: u32,
        voting_period: u32,
        proposal_threshold: u128,
        quorum_bps: u32,
    ) {
        if quorum_bps == 0 || quorum_bps as u128 > BPS {
            panic_with_error!(e, RatifyGovernorError::InvalidQuorum);
        }
        stellar_governance::governor::set_name(e, name);
        stellar_governance::governor::set_token_contract(e, &weight_rule);
        stellar_governance::governor::set_voting_delay(e, voting_delay);
        stellar_governance::governor::set_voting_period(e, voting_period);
        stellar_governance::governor::set_proposal_threshold(e, proposal_threshold);
        e.storage().instance().set(&GovStorageKey::Timelock, &timelock);
        e.storage().instance().set(&GovStorageKey::QuorumBps, &quorum_bps);
        if let Some(registry) = registry {
            e.storage().instance().set(&GovStorageKey::Registry, &registry);
        }
    }

    // ################## QUERIES ##################

    /// Returns the timelock approved actions are queued into.
    pub fn timelock(e: &Env) -> Address {
        e.storage()
            .instance()
            .get(&GovStorageKey::Timelock)
            .unwrap_or_else(|| panic_with_error!(e, RatifyGovernorError::NotConfigured))
    }

    /// Returns the delegate registry, if the community has one.
    pub fn registry(e: &Env) -> Option<Address> {
        e.storage().instance().get(&GovStorageKey::Registry)
    }

    /// Returns the share of live voting power a proposal must reach, in
    /// hundredths of a percent.
    pub fn quorum_bps(e: &Env) -> u32 {
        e.storage().instance().get(&GovStorageKey::QuorumBps).unwrap_or(0)
    }

    /// Returns the vote tallies for a proposal.
    ///
    /// The governance library keeps these but the standard governor interface
    /// does not expose them, and both the proposal page and the delegate
    /// registry need to read them.
    pub fn get_proposal_vote_counts(
        e: &Env,
        proposal_id: BytesN<32>,
    ) -> stellar_governance::governor::ProposalVoteCounts {
        stellar_governance::governor::get_proposal_vote_counts(e, &proposal_id)
    }

    /// Returns the voting power that has been cast on a proposal against the
    /// quorum it has to reach.
    ///
    /// The proposal page shows this as progress, so a member can see whether
    /// a vote is short of a quorum rather than short of support. They are
    /// different problems and they need different responses.
    pub fn quorum_progress(e: &Env, proposal_id: BytesN<32>) -> (u128, u128) {
        let snapshot = Self::proposal_snapshot(e, proposal_id.clone());
        let counts = stellar_governance::governor::get_proposal_vote_counts(e, &proposal_id);
        let reached = counts.for_votes + counts.abstain_votes;
        (reached, Self::quorum(e, snapshot))
    }

    /// Returns the identifier the timelock will hold an action under.
    ///
    /// The queue page reads this to line a queued item up against the
    /// proposal that put it there.
    pub fn operation_id(
        e: &Env,
        targets: Vec<Address>,
        functions: Vec<Symbol>,
        args: Vec<Vec<Val>>,
        description_hash: BytesN<32>,
        index: u32,
    ) -> BytesN<32> {
        let proposal_id = hash_proposal(e, &targets, &functions, &args, &description_hash);
        let operations = Self::operations(e, &proposal_id, &targets, &functions, &args);
        hash_operation(e, &operations.get(index).unwrap())
    }

    // ################## THE BRAKE ##################

    /// Stops an approved proposal, with a reason recorded on chain.
    ///
    /// The guardian's power, and the guardian is the timelock's, not the
    /// governor's, so there is one holder of the brake for the whole
    /// community. Everything waiting in the timelock for this proposal is
    /// cancelled with it, so nothing is left executable behind the governor's
    /// back.
    pub fn stop(
        e: &Env,
        targets: Vec<Address>,
        functions: Vec<Symbol>,
        args: Vec<Vec<Val>>,
        description_hash: BytesN<32>,
        guardian: Address,
        reason: String,
    ) -> BytesN<32> {
        let timelock = Self::timelock(e);
        let expected = GuardianClient::new(e, &timelock).guardian();
        if guardian != expected {
            panic_with_error!(e, RatifyGovernorError::NotGuardian);
        }
        guardian.require_auth();
        if reason.len() == 0 {
            panic_with_error!(e, RatifyGovernorError::ReasonRequired);
        }
        if reason.len() > MAX_REASON_LENGTH {
            panic_with_error!(e, RatifyGovernorError::ReasonTooLong);
        }

        let proposal_id = hash_proposal(e, &targets, &functions, &args, &description_hash);
        let state = Self::proposal_state(e, proposal_id.clone());

        // Anything already in the timelock has to come out too, or it stays
        // executable regardless of what the governor thinks.
        if state == ProposalState::Queued {
            let brake = GuardianClient::new(e, &timelock);
            for operation in
                Self::operations(e, &proposal_id, &targets, &functions, &args).iter()
            {
                brake.cancel_with_reason(
                    &hash_operation(e, &operation),
                    &guardian,
                    &reason,
                );
            }
        } else if state != ProposalState::Pending
            && state != ProposalState::Active
            && state != ProposalState::Succeeded
        {
            panic_with_error!(e, RatifyGovernorError::WrongState);
        }

        stellar_governance::governor::cancel(e, targets, functions, args, &description_hash);
        ProposalStoppedWithReason { proposal_id: proposal_id.clone(), guardian, reason }
            .publish(e);
        proposal_id
    }

    // ################## INTERNAL ##################

    /// Builds the timelock operations a proposal's actions become.
    ///
    /// One operation per action, chained so they can only run in the order the
    /// proposal set out. The salt is derived from the proposal, so two
    /// proposals that happen to contain the same call are still distinct
    /// operations and neither can execute the other.
    fn operations(
        e: &Env,
        proposal_id: &BytesN<32>,
        targets: &Vec<Address>,
        functions: &Vec<Symbol>,
        args: &Vec<Vec<Val>>,
    ) -> Vec<Operation> {
        if targets.len() > MAX_ACTIONS {
            panic_with_error!(e, RatifyGovernorError::TooManyActions);
        }

        let mut operations: Vec<Operation> = Vec::new(e);
        let mut predecessor = BytesN::from_array(e, &[0u8; 32]);

        for i in 0..targets.len() {
            let operation = Operation {
                target: targets.get_unchecked(i),
                function: functions.get_unchecked(i),
                args: args.get_unchecked(i),
                predecessor: predecessor.clone(),
                salt: Self::salt(e, proposal_id, i),
            };
            predecessor = hash_operation(e, &operation);
            operations.push_back(operation);
        }

        operations
    }

    fn salt(e: &Env, proposal_id: &BytesN<32>, index: u32) -> BytesN<32> {
        let mut bytes = Bytes::from_array(e, &proposal_id.to_array());
        bytes.extend_from_array(&index.to_be_bytes());
        e.crypto().keccak256(&bytes).to_bytes()
    }

    /// Marks a proposal executed without invoking anything.
    ///
    /// The library's own execute would call the targets directly, which would
    /// walk straight past the timelock. The calls have already been made
    /// through the timelock by the time this runs; all that is left is the
    /// state.
    fn mark_executed(e: &Env, proposal_id: &BytesN<32>) {
        let mut core: ProposalCore =
            stellar_governance::governor::get_proposal_core(e, proposal_id);
        core.state = ProposalState::Executed;
        e.storage()
            .persistent()
            .set(&GovernorStorageKey::Proposal(proposal_id.clone()), &core);
        emit_proposal_executed(e, proposal_id);
    }
}

#[contractimpl(contracttrait)]
impl Governor for RatifyGovernor {
    /// Returns the voting power a proposal must reach at a ledger.
    ///
    /// A share of the power that was live then, rather than a fixed number.
    /// Because delegation lapses, the denominator reflects members who
    /// confirmed within their term, so a community can set a real quorum
    /// without it drifting out of reach.
    ///
    /// A ledger with no live power at all returns an unreachable quorum. That
    /// only happens before a community has any active members, and it is the
    /// right answer: a community with nobody present cannot decide anything.
    fn quorum(e: &Env, ledger: u32) -> u128 {
        let rule = stellar_governance::governor::get_token_contract(e);
        let live = VotesClient::new(e, &rule).get_total_supply_at_checkpoint(&ledger);
        if live == 0 {
            return u128::MAX;
        }
        let bps = Self::quorum_bps(e) as u128;
        // Round up, so a quorum of a tenth of eleven is two rather than one.
        live.saturating_mul(bps).div_ceil(BPS)
    }

    /// Proposals must be queued before they can run.
    fn proposals_need_queuing(_e: &Env) -> bool {
        true
    }

    /// Creates a proposal and tells the delegate registry it exists.
    ///
    /// Registering the proposal at creation is what lets the registry settle
    /// it afterwards and record who was eligible but stayed away.
    fn propose(
        e: &Env,
        targets: Vec<Address>,
        functions: Vec<Symbol>,
        args: Vec<Vec<Val>>,
        description: String,
        proposer: Address,
    ) -> BytesN<32> {
        if targets.len() > MAX_ACTIONS {
            panic_with_error!(e, RatifyGovernorError::TooManyActions);
        }
        proposer.require_auth();
        let proposal_id = stellar_governance::governor::propose(
            e,
            targets,
            functions,
            args,
            description,
            &proposer,
        );

        if let Some(registry) = Self::registry(e) {
            let snapshot =
                stellar_governance::governor::get_proposal_snapshot(e, &proposal_id);
            let deadline =
                stellar_governance::governor::get_proposal_deadline(e, &proposal_id);
            RegistryClient::new(e, &registry).open(&proposal_id, &snapshot, &deadline);
        }

        proposal_id
    }

    /// Casts a vote and records it against the voter's public record.
    ///
    /// The weight comes from the community's weight rule at the proposal's
    /// snapshot, so power acquired after a proposal opened cannot change its
    /// outcome, and a grant that has lapsed since is still worth what it was
    /// worth then.
    fn cast_vote(
        e: &Env,
        proposal_id: BytesN<32>,
        vote_type: u32,
        reason: String,
        voter: Address,
    ) -> u128 {
        voter.require_auth();
        let snapshot =
            stellar_governance::governor::get_proposal_snapshot(e, &proposal_id);
        let quorum = Self::quorum(e, snapshot);
        let weight = stellar_governance::governor::cast_vote(
            e,
            &proposal_id,
            vote_type,
            &reason,
            &voter,
            quorum,
        );

        if let Some(registry) = Self::registry(e) {
            let ballot = match vote_type {
                0 => Ballot::Against,
                1 => Ballot::For,
                _ => Ballot::Abstain,
            };
            RegistryClient::new(e, &registry).record_vote(
                &voter,
                &proposal_id,
                &ballot,
                &weight,
            );
        }

        weight
    }

    /// Puts an approved proposal into the timelock.
    ///
    /// Anyone may queue a proposal that has succeeded. There is nothing to
    /// decide at this point and no reason to make the community wait on one
    /// account being willing.
    ///
    /// The delay comes from the timelock, not from the caller, so the `eta`
    /// argument of the standard interface is ignored rather than trusted.
    fn queue(
        e: &Env,
        targets: Vec<Address>,
        functions: Vec<Symbol>,
        args: Vec<Vec<Val>>,
        description_hash: BytesN<32>,
        _eta: u32,
        _operator: Address,
    ) -> BytesN<32> {
        let proposal_id = hash_proposal(e, &targets, &functions, &args, &description_hash);
        let snapshot = stellar_governance::governor::get_proposal_snapshot(e, &proposal_id);
        let quorum = Self::quorum(e, snapshot);

        let timelock = Self::timelock(e);
        let client = TimelockClient::new(e, &timelock);
        let delay = client.get_min_delay();
        let eta = e.ledger().sequence().saturating_add(delay);

        // Move the proposal to Queued first. It refuses anything that has not
        // succeeded, so nothing reaches the timelock that the vote did not
        // approve.
        let queued = stellar_governance::governor::queue(
            e,
            targets.clone(),
            functions.clone(),
            args.clone(),
            &description_hash,
            eta,
            quorum,
        );

        for operation in Self::operations(e, &proposal_id, &targets, &functions, &args).iter() {
            client.schedule(
                &operation.target,
                &operation.function,
                &operation.args,
                &operation.predecessor,
                &operation.salt,
                &delay,
                &e.current_contract_address(),
            );
        }

        queued
    }

    /// Runs an approved proposal whose delay has passed.
    ///
    /// Execution is open, so a decision that has been approved and has waited
    /// does not need anyone's permission to take effect. Every call goes
    /// through the timelock, which is what makes executing early impossible
    /// and executing twice impossible.
    fn execute(
        e: &Env,
        targets: Vec<Address>,
        functions: Vec<Symbol>,
        args: Vec<Vec<Val>>,
        description_hash: BytesN<32>,
        _executor: Address,
    ) -> BytesN<32> {
        let proposal_id = hash_proposal(e, &targets, &functions, &args, &description_hash);

        match Self::proposal_state(e, proposal_id.clone()) {
            ProposalState::Queued => {}
            ProposalState::Executed => {
                panic_with_error!(
                    e,
                    stellar_governance::governor::GovernorError::ProposalAlreadyExecuted
                )
            }
            _ => panic_with_error!(
                e,
                stellar_governance::governor::GovernorError::ProposalNotQueued
            ),
        }

        let client = TimelockClient::new(e, &Self::timelock(e));
        for operation in Self::operations(e, &proposal_id, &targets, &functions, &args).iter() {
            client.execute(
                &operation.target,
                &operation.function,
                &operation.args,
                &operation.predecessor,
                &operation.salt,
                &None,
            );
        }

        Self::mark_executed(e, &proposal_id);
        proposal_id
    }

    /// Withdraws a proposal before voting opens.
    ///
    /// The proposer's own, and only while the proposal is still Pending. Once
    /// members have started voting, withdrawing is not the proposer's to
    /// decide; stopping it is the guardian's job and carries a reason. See
    /// [`RatifyGovernor::stop`].
    fn cancel(
        e: &Env,
        targets: Vec<Address>,
        functions: Vec<Symbol>,
        args: Vec<Vec<Val>>,
        description_hash: BytesN<32>,
        operator: Address,
    ) -> BytesN<32> {
        let proposal_id = hash_proposal(e, &targets, &functions, &args, &description_hash);
        let proposer = stellar_governance::governor::get_proposal_proposer(e, &proposal_id);
        if operator != proposer {
            panic_with_error!(e, RatifyGovernorError::NotProposer);
        }
        operator.require_auth();

        if Self::proposal_state(e, proposal_id.clone()) != ProposalState::Pending {
            panic_with_error!(e, RatifyGovernorError::WrongState);
        }

        let cancelled =
            stellar_governance::governor::cancel(e, targets, functions, args, &description_hash);
        emit_proposal_cancelled(e, &cancelled);
        cancelled
    }
}
