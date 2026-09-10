#![cfg(test)]
extern crate std;

use ratify_delegate_registry::{RatifyDelegateRegistry, RatifyDelegateRegistryClient};
use ratify_membership::{RatifyMembership, RatifyMembershipClient};
use ratify_timelock::{RatifyTimelock, RatifyTimelockClient};
use ratify_treasury::{RatifyTreasury, RatifyTreasuryClient};
use ratify_weight_rule::{RatifyWeightRule, WeightModel};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger as _},
    token, Address, BytesN, Env, Error, IntoVal, InvokeError, String, Symbol, Val, Vec,
};
use stellar_governance::{
    governor::{GovernorClient, ProposalState},
    timelock::{OperationState, TimelockClient},
};

use crate::{RatifyGovernor, RatifyGovernorClient, RatifyGovernorError};

const START: u32 = 10_000;
const VOTING_DELAY: u32 = 20;
const VOTING_PERIOD: u32 = 200;
const TIMELOCK_DELAY: u32 = 500;
const TERM: u32 = 100_000;
/// A fifth of live voting power has to take part.
const QUORUM_BPS: u32 = 2_000;
const CONTESTED_BPS: u32 = 2_000;

struct Fixture<'a> {
    e: Env,
    founder: Address,
    guardian: Address,
    membership: RatifyMembershipClient<'a>,
    treasury: RatifyTreasuryClient<'a>,
    treasury_id: Address,
    timelock: RatifyTimelockClient<'a>,
    timelock_id: Address,
    registry: RatifyDelegateRegistryClient<'a>,
    ratify: RatifyGovernorClient<'a>,
    /// The governor seen through the standard interface, which is how a
    /// member proposes, votes, queues and executes.
    gov: GovernorClient<'a>,
    asset: Address,
    token: token::TokenClient<'a>,
}

/// A whole community: membership, weighting, governor, timelock, treasury and
/// the delegate registry, wired the way the factory will wire them.
fn setup<'a>() -> Fixture<'a> {
    let e = Env::default();
    e.mock_all_auths();
    e.ledger().set_sequence_number(START);

    let founder = Address::generate(&e);
    let guardian = Address::generate(&e);

    // The governor and the timelock each need the other's address, so the
    // governor's is settled first and it is deployed there afterwards. The
    // factory does the same thing with a computed address.
    let governor_id = Address::generate(&e);

    let membership_id = e.register(
        RatifyMembership,
        (
            founder.clone(),
            String::from_str(&e, "Riverside Commons"),
            String::from_str(&e, "RIVER"),
            String::from_str(&e, "https://example.invalid/member/"),
        ),
    );
    let weight_rule_id = e.register(
        RatifyWeightRule,
        (
            membership_id.clone(),
            WeightModel::OneTokenOneVote,
            Option::<ratify_weight_rule::TenureSchedule>::None,
        ),
    );
    let timelock_id = e.register(
        RatifyTimelock,
        (governor_id.clone(), guardian.clone(), TIMELOCK_DELAY),
    );
    let treasury_id = e.register(RatifyTreasury, (timelock_id.clone(),));
    let registry_id = e.register(
        RatifyDelegateRegistry,
        (governor_id.clone(), weight_rule_id.clone(), CONTESTED_BPS),
    );

    e.register_at(
        &governor_id,
        RatifyGovernor,
        (
            weight_rule_id,
            timelock_id.clone(),
            Some(registry_id.clone()),
            String::from_str(&e, "Riverside Commons"),
            VOTING_DELAY,
            VOTING_PERIOD,
            1u128,
            QUORUM_BPS,
        ),
    );

    // Fund the treasury and let it spend, which in a running community are
    // both proposals. Seeded here so a test can be about one thing.
    let issuer = Address::generate(&e);
    let asset = e.register_stellar_asset_contract_v2(issuer).address();
    token::StellarAssetClient::new(&e, &asset).mint(&treasury_id, &1_000_000);
    RatifyTreasuryClient::new(&e, &treasury_id).set_policy(&asset, &10_000, &50_000, &100_000);

    Fixture {
        membership: RatifyMembershipClient::new(&e, &membership_id),
        treasury: RatifyTreasuryClient::new(&e, &treasury_id),
        timelock: RatifyTimelockClient::new(&e, &timelock_id),
        registry: RatifyDelegateRegistryClient::new(&e, &registry_id),
        ratify: RatifyGovernorClient::new(&e, &governor_id),
        gov: GovernorClient::new(&e, &governor_id),
        token: token::TokenClient::new(&e, &asset),
        treasury_id,
        timelock_id,
        founder,
        guardian,
        asset,
        e,
    }
}

fn err(code: RatifyGovernorError) -> Result<Error, InvokeError> {
    Ok(Error::from_contract_error(code as u32))
}

/// A member holding `tokens` tokens with a live grant to themselves.
fn member(f: &Fixture, tokens: u32) -> Address {
    let account = Address::generate(&f.e);
    for _ in 0..tokens {
        f.membership.issue(&account);
    }
    f.membership.delegate_for(&account, &account, &TERM);
    account
}

/// A proposal: pay `amount` of the community's asset to `to`.
struct Payment {
    targets: Vec<Address>,
    functions: Vec<Symbol>,
    args: Vec<Vec<Val>>,
    description: String,
    description_hash: BytesN<32>,
}

fn payment(f: &Fixture, to: &Address, amount: i128, text: &str) -> Payment {
    let e = &f.e;
    let description = String::from_str(e, text);
    let args: Vec<Val> = Vec::from_array(
        e,
        [
            f.asset.clone().into_val(e),
            to.clone().into_val(e),
            amount.into_val(e),
            BytesN::from_array(e, &[0u8; 32]).into_val(e),
        ],
    );
    Payment {
        targets: Vec::from_array(e, [f.treasury_id.clone()]),
        functions: Vec::from_array(e, [symbol_short!("pay")]),
        args: Vec::from_array(e, [args]),
        description_hash: e.crypto().keccak256(&description.to_bytes()).to_bytes(),
        description,
    }
}

fn propose(f: &Fixture, p: &Payment, proposer: &Address) -> BytesN<32> {
    f.gov
        .propose(&p.targets, &p.functions, &p.args, &p.description, proposer)
}

fn queue(f: &Fixture, p: &Payment, by: &Address) -> BytesN<32> {
    f.gov.queue(
        &p.targets,
        &p.functions,
        &p.args,
        &p.description_hash,
        &0,
        by,
    )
}

fn execute(f: &Fixture, p: &Payment, by: &Address) -> BytesN<32> {
    f.gov
        .execute(&p.targets, &p.functions, &p.args, &p.description_hash, by)
}

/// Moves to a ledger inside the voting window of a proposal made now.
fn open_voting(f: &Fixture, proposed_at: u32) {
    f.e.ledger()
        .set_sequence_number(proposed_at + VOTING_DELAY + 1);
}

/// Moves past the end of the voting window.
fn close_voting(f: &Fixture, proposed_at: u32) {
    f.e.ledger()
        .set_sequence_number(proposed_at + VOTING_DELAY + VOTING_PERIOD + 1);
}

// ################## THE WHOLE PATH ##################

#[test]
fn a_vote_ends_in_a_payment() {
    let f = setup();
    let alice = member(&f, 3);
    let bob = member(&f, 2);
    let recipient = Address::generate(&f.e);

    f.e.ledger().set_sequence_number(START + 1);
    let proposed_at = START + 1;
    let p = payment(&f, &recipient, 4_000, "Pay the roofer.");
    let id = propose(&f, &p, &alice);

    assert_eq!(f.gov.proposal_state(&id), ProposalState::Pending);
    assert_eq!(f.treasury.balance(&f.asset), 1_000_000);

    open_voting(&f, proposed_at);
    assert_eq!(f.gov.proposal_state(&id), ProposalState::Active);
    f.gov
        .cast_vote(&id, &1, &String::from_str(&f.e, "The roof leaks."), &alice);
    f.gov.cast_vote(&id, &1, &String::from_str(&f.e, ""), &bob);

    close_voting(&f, proposed_at);
    assert_eq!(f.gov.proposal_state(&id), ProposalState::Succeeded);

    assert_eq!(f.treasury.balance(&f.asset), 1_000_000);

    queue(&f, &p, &alice);
    assert_eq!(f.gov.proposal_state(&id), ProposalState::Queued);
    assert_eq!(f.treasury.balance(&f.asset), 1_000_000);

    assert!(f
        .gov
        .try_execute(
            &p.targets,
            &p.functions,
            &p.args,
            &p.description_hash,
            &alice
        )
        .is_err());
    assert_eq!(f.token.balance(&recipient), 0);

    let stranger = Address::generate(&f.e);
    f.e.ledger()
        .set_sequence_number(f.e.ledger().sequence() + TIMELOCK_DELAY);
    execute(&f, &p, &stranger);

    assert_eq!(f.gov.proposal_state(&id), ProposalState::Executed);
    assert_eq!(f.token.balance(&recipient), 4_000);
    assert_eq!(f.treasury.balance(&f.asset), 996_000);

    assert_eq!(f.treasury.outflow_count(), 1);
    let outflow = f.treasury.outflow(&0);
    assert_eq!(outflow.to, recipient);
    assert_eq!(outflow.amount, 4_000);
}

#[test]
fn executing_twice_pays_once() {
    let f = setup();
    let alice = member(&f, 3);
    let recipient = Address::generate(&f.e);
    f.e.ledger().set_sequence_number(START + 1);
    let proposed_at = START + 1;
    let p = payment(&f, &recipient, 1_000, "Once.");

    propose(&f, &p, &alice);
    open_voting(&f, proposed_at);
    f.gov
        .cast_vote(&p_id(&f, &p), &1, &String::from_str(&f.e, ""), &alice);
    close_voting(&f, proposed_at);
    queue(&f, &p, &alice);
    f.e.ledger()
        .set_sequence_number(f.e.ledger().sequence() + TIMELOCK_DELAY);
    execute(&f, &p, &alice);

    assert!(f
        .gov
        .try_execute(
            &p.targets,
            &p.functions,
            &p.args,
            &p.description_hash,
            &alice
        )
        .is_err());
    assert_eq!(f.token.balance(&recipient), 1_000);
}

#[test]
fn a_proposal_that_was_not_queued_cannot_run() {
    let f = setup();
    let alice = member(&f, 3);
    let recipient = Address::generate(&f.e);
    f.e.ledger().set_sequence_number(START + 1);
    let proposed_at = START + 1;
    let p = payment(&f, &recipient, 1_000, "Skip the queue.");

    propose(&f, &p, &alice);
    open_voting(&f, proposed_at);
    f.gov
        .cast_vote(&p_id(&f, &p), &1, &String::from_str(&f.e, ""), &alice);
    close_voting(&f, proposed_at);

    assert_eq!(
        f.gov.proposal_state(&p_id(&f, &p)),
        ProposalState::Succeeded
    );
    assert!(f
        .gov
        .try_execute(
            &p.targets,
            &p.functions,
            &p.args,
            &p.description_hash,
            &alice
        )
        .is_err());
    assert_eq!(f.token.balance(&recipient), 0);
}

#[test]
fn a_proposal_that_lost_cannot_be_queued() {
    let f = setup();
    let alice = member(&f, 3);
    let bob = member(&f, 5);
    let recipient = Address::generate(&f.e);
    f.e.ledger().set_sequence_number(START + 1);
    let proposed_at = START + 1;
    let p = payment(&f, &recipient, 1_000, "No thanks.");

    propose(&f, &p, &alice);
    open_voting(&f, proposed_at);
    f.gov
        .cast_vote(&p_id(&f, &p), &1, &String::from_str(&f.e, ""), &alice);
    f.gov
        .cast_vote(&p_id(&f, &p), &0, &String::from_str(&f.e, ""), &bob);
    close_voting(&f, proposed_at);

    assert_eq!(f.gov.proposal_state(&p_id(&f, &p)), ProposalState::Defeated);
    assert!(f
        .gov
        .try_queue(
            &p.targets,
            &p.functions,
            &p.args,
            &p.description_hash,
            &0,
            &alice
        )
        .is_err());
}

// ################## QUORUM ##################

#[test]
fn a_proposal_nobody_much_voted_on_does_not_pass() {
    let f = setup();
    let alice = member(&f, 1);
    let _rest = member(&f, 9);
    let recipient = Address::generate(&f.e);
    f.e.ledger().set_sequence_number(START + 1);
    let proposed_at = START + 1;
    let p = payment(&f, &recipient, 1_000, "One voice.");

    propose(&f, &p, &alice);
    open_voting(&f, proposed_at);
    f.gov
        .cast_vote(&p_id(&f, &p), &1, &String::from_str(&f.e, ""), &alice);

    let (reached, needed) = f.ratify.quorum_progress(&p_id(&f, &p));
    assert_eq!(reached, 1);
    assert_eq!(needed, 2);

    close_voting(&f, proposed_at);
    assert_eq!(f.gov.proposal_state(&p_id(&f, &p)), ProposalState::Defeated);
}

#[test]
fn quorum_is_measured_against_power_that_is_still_live() {
    let f = setup();
    let alice = member(&f, 1);
    let absent = member(&f, 9);

    f.e.ledger().set_sequence_number(START + TERM - 1);
    f.membership.renew(&alice, &TERM);

    f.e.ledger().set_sequence_number(START + TERM);
    f.membership.lapse(&absent);
    assert_eq!(f.membership.live_total(), 1);

    let recipient = Address::generate(&f.e);
    f.e.ledger().set_sequence_number(START + TERM + 1);
    let proposed_at = START + TERM + 1;
    let p = payment(&f, &recipient, 1_000, "Just us now.");

    propose(&f, &p, &alice);
    open_voting(&f, proposed_at);
    f.gov
        .cast_vote(&p_id(&f, &p), &1, &String::from_str(&f.e, ""), &alice);

    let (reached, needed) = f.ratify.quorum_progress(&p_id(&f, &p));
    assert_eq!(reached, 1);
    assert_eq!(needed, 1);

    close_voting(&f, proposed_at);
    assert_eq!(
        f.gov.proposal_state(&p_id(&f, &p)),
        ProposalState::Succeeded
    );
}

#[test]
fn power_acquired_after_a_proposal_opened_does_not_count() {
    let f = setup();
    let alice = member(&f, 1);
    let latecomer = Address::generate(&f.e);
    f.e.ledger().set_sequence_number(START + 1);
    let proposed_at = START + 1;
    let p = payment(&f, &Address::generate(&f.e), 1_000, "Too late.");
    propose(&f, &p, &alice);

    open_voting(&f, proposed_at);
    for _ in 0..100 {
        f.membership.issue(&latecomer);
    }
    f.membership.delegate_for(&latecomer, &latecomer, &TERM);

    assert_eq!(
        f.gov
            .cast_vote(&p_id(&f, &p), &0, &String::from_str(&f.e, ""), &latecomer),
        0
    );
}

// ################## THE BRAKE ##################

#[test]
fn the_guardian_can_stop_an_approved_proposal() {
    let f = setup();
    let alice = member(&f, 3);
    let recipient = Address::generate(&f.e);
    f.e.ledger().set_sequence_number(START + 1);
    let proposed_at = START + 1;
    let p = payment(&f, &recipient, 1_000, "Wrong address.");

    let id = propose(&f, &p, &alice);
    open_voting(&f, proposed_at);
    f.gov
        .cast_vote(&id, &1, &String::from_str(&f.e, ""), &alice);
    close_voting(&f, proposed_at);
    queue(&f, &p, &alice);

    f.ratify.stop(
        &p.targets,
        &p.functions,
        &p.args,
        &p.description_hash,
        &f.guardian,
        &String::from_str(&f.e, "The recipient address is wrong."),
    );

    assert_eq!(f.gov.proposal_state(&id), ProposalState::Canceled);

    let operation =
        f.ratify
            .operation_id(&p.targets, &p.functions, &p.args, &p.description_hash, &0);
    let timelock = TimelockClient::new(&f.e, &f.timelock_id);
    assert_eq!(
        timelock.get_operation_state(&operation),
        OperationState::Unset
    );

    f.e.ledger()
        .set_sequence_number(f.e.ledger().sequence() + TIMELOCK_DELAY);
    assert!(f
        .gov
        .try_execute(
            &p.targets,
            &p.functions,
            &p.args,
            &p.description_hash,
            &alice
        )
        .is_err());
    assert_eq!(f.token.balance(&recipient), 0);
}

#[test]
fn nobody_but_the_guardian_can_stop_one() {
    let f = setup();
    let alice = member(&f, 3);
    f.e.ledger().set_sequence_number(START + 1);
    let p = payment(&f, &Address::generate(&f.e), 1_000, "Mine.");
    propose(&f, &p, &alice);

    let reason = String::from_str(&f.e, "I changed my mind.");
    assert_eq!(
        f.ratify.try_stop(
            &p.targets,
            &p.functions,
            &p.args,
            &p.description_hash,
            &alice,
            &reason
        ),
        Err(err(RatifyGovernorError::NotGuardian))
    );
}

#[test]
fn stopping_a_proposal_must_state_a_reason() {
    let f = setup();
    let alice = member(&f, 3);
    f.e.ledger().set_sequence_number(START + 1);
    let p = payment(&f, &Address::generate(&f.e), 1_000, "Why?");
    propose(&f, &p, &alice);

    assert_eq!(
        f.ratify.try_stop(
            &p.targets,
            &p.functions,
            &p.args,
            &p.description_hash,
            &f.guardian,
            &String::from_str(&f.e, "")
        ),
        Err(err(RatifyGovernorError::ReasonRequired))
    );
}

#[test]
fn a_proposer_may_withdraw_before_voting_opens_and_not_after() {
    let f = setup();
    let alice = member(&f, 3);
    f.e.ledger().set_sequence_number(START + 1);
    let proposed_at = START + 1;
    let p = payment(&f, &Address::generate(&f.e), 1_000, "Second thoughts.");
    let id = propose(&f, &p, &alice);

    let bob = member(&f, 1);
    assert_eq!(
        f.gov
            .try_cancel(&p.targets, &p.functions, &p.args, &p.description_hash, &bob),
        Err(err(RatifyGovernorError::NotProposer))
    );

    f.gov.cancel(
        &p.targets,
        &p.functions,
        &p.args,
        &p.description_hash,
        &alice,
    );
    assert_eq!(f.gov.proposal_state(&id), ProposalState::Canceled);

    let q = payment(
        &f,
        &Address::generate(&f.e),
        2_000,
        "Too late for this one.",
    );
    propose(&f, &q, &alice);
    open_voting(&f, proposed_at);
    assert_eq!(
        f.gov.try_cancel(
            &q.targets,
            &q.functions,
            &q.args,
            &q.description_hash,
            &alice
        ),
        Err(err(RatifyGovernorError::WrongState))
    );
}

// ################## THE TREASURY HAS THE LAST WORD ##################

#[test]
fn a_vote_cannot_authorise_what_the_policy_forbids() {
    let f = setup();
    let alice = member(&f, 3);
    let recipient = Address::generate(&f.e);
    f.e.ledger().set_sequence_number(START + 1);
    let proposed_at = START + 1;

    let p = payment(&f, &recipient, 20_000, "More than the policy allows.");
    propose(&f, &p, &alice);
    open_voting(&f, proposed_at);
    f.gov
        .cast_vote(&p_id(&f, &p), &1, &String::from_str(&f.e, ""), &alice);
    close_voting(&f, proposed_at);
    queue(&f, &p, &alice);
    f.e.ledger()
        .set_sequence_number(f.e.ledger().sequence() + TIMELOCK_DELAY);

    assert!(f
        .gov
        .try_execute(
            &p.targets,
            &p.functions,
            &p.args,
            &p.description_hash,
            &alice
        )
        .is_err());
    assert_eq!(f.token.balance(&recipient), 0);
    assert_eq!(f.treasury.balance(&f.asset), 1_000_000);
}

#[test]
fn the_governor_cannot_reach_the_treasury_on_its_own() {
    let f = setup();
    assert_eq!(f.treasury.timelock(), f.timelock_id);
    assert_eq!(f.timelock.governor(), f.ratify.address);
}

// ################## THE DELEGATE RECORD ##################

#[test]
fn a_vote_reaches_the_delegate_record() {
    let f = setup();
    let alice = member(&f, 3);
    let absentee = member(&f, 2);
    f.e.ledger().set_sequence_number(START + 1);
    let proposed_at = START + 1;
    let p = payment(&f, &Address::generate(&f.e), 1_000, "Turnout test.");
    let id = propose(&f, &p, &alice);

    let recorded = f.registry.proposal(&id).unwrap();
    assert_eq!(recorded.snapshot, proposed_at + VOTING_DELAY);

    open_voting(&f, proposed_at);
    f.gov
        .cast_vote(&id, &1, &String::from_str(&f.e, ""), &alice);
    close_voting(&f, proposed_at);

    f.registry.settle(&alice, &id);
    f.registry.settle(&absentee, &id);

    assert_eq!(f.registry.participation_bps(&alice), Some(10_000));
    assert_eq!(f.registry.participation_bps(&absentee), Some(0));
}

// ################## PROPOSING ##################

#[test]
fn a_member_below_the_threshold_cannot_propose() {
    let f = setup();
    let alice = member(&f, 1);
    let onlooker = Address::generate(&f.e);
    f.membership.issue(&onlooker);
    f.e.ledger().set_sequence_number(START + 1);

    let p = payment(&f, &Address::generate(&f.e), 1_000, "From nowhere.");
    assert!(f
        .gov
        .try_propose(&p.targets, &p.functions, &p.args, &p.description, &onlooker)
        .is_err());

    propose(&f, &p, &alice);
}

#[test]
fn the_same_proposal_cannot_be_made_twice() {
    let f = setup();
    let alice = member(&f, 3);
    f.e.ledger().set_sequence_number(START + 1);
    let p = payment(&f, &Address::generate(&f.e), 1_000, "Twice.");
    propose(&f, &p, &alice);

    assert!(f
        .gov
        .try_propose(&p.targets, &p.functions, &p.args, &p.description, &alice)
        .is_err());
}

#[test]
fn a_member_cannot_vote_twice() {
    let f = setup();
    let alice = member(&f, 3);
    f.e.ledger().set_sequence_number(START + 1);
    let proposed_at = START + 1;
    let p = payment(&f, &Address::generate(&f.e), 1_000, "Once each.");
    let id = propose(&f, &p, &alice);

    open_voting(&f, proposed_at);
    f.gov
        .cast_vote(&id, &1, &String::from_str(&f.e, ""), &alice);
    assert!(f
        .gov
        .try_cast_vote(&id, &0, &String::from_str(&f.e, ""), &alice)
        .is_err());
}

#[test]
fn voting_before_it_opens_or_after_it_closes_is_refused() {
    let f = setup();
    let alice = member(&f, 3);
    f.e.ledger().set_sequence_number(START + 1);
    let proposed_at = START + 1;
    let p = payment(&f, &Address::generate(&f.e), 1_000, "Timing.");
    let id = propose(&f, &p, &alice);

    assert!(f
        .gov
        .try_cast_vote(&id, &1, &String::from_str(&f.e, ""), &alice)
        .is_err());

    close_voting(&f, proposed_at);
    assert!(f
        .gov
        .try_cast_vote(&id, &1, &String::from_str(&f.e, ""), &alice)
        .is_err());
}

/// The identifier of a proposal, recomputed from its parts.
fn p_id(f: &Fixture, p: &Payment) -> BytesN<32> {
    f.gov
        .get_proposal_id(&p.targets, &p.functions, &p.args, &p.description_hash)
}
