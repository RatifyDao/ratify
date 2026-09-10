#![cfg(test)]
extern crate std;

use ratify_membership::{RatifyMembership, RatifyMembershipClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, Error, InvokeError, String,
};
use stellar_governance::votes::VotesClient;

use crate::{
    RatifyWeightRule, RatifyWeightRuleClient, TenureSchedule, WeightModel, WeightRuleError,
    MAX_TENURE_STEPS,
};

const START: u32 = 10_000;
const TERM: u32 = 50_000;
const STEP: u32 = 1_000;

struct Fixture<'a> {
    e: Env,
    membership: RatifyMembershipClient<'a>,
    rule: RatifyWeightRuleClient<'a>,
    /// The rule seen through the standard votes interface, which is how the
    /// governor reaches it.
    votes: VotesClient<'a>,
}

fn setup<'a>(model: WeightModel) -> Fixture<'a> {
    setup_with(model, None)
}

fn setup_with<'a>(model: WeightModel, tenure: Option<TenureSchedule>) -> Fixture<'a> {
    let e = Env::default();
    e.mock_all_auths();
    e.ledger().set_sequence_number(START);

    let issuer = Address::generate(&e);
    let membership_id = e.register(
        RatifyMembership,
        (
            issuer,
            String::from_str(&e, "Riverside Commons"),
            String::from_str(&e, "RIVER"),
            String::from_str(&e, "https://example.invalid/member/"),
        ),
    );
    let rule_id = e.register(RatifyWeightRule, (membership_id.clone(), model, tenure));

    Fixture {
        membership: RatifyMembershipClient::new(&e, &membership_id),
        rule: RatifyWeightRuleClient::new(&e, &rule_id),
        votes: VotesClient::new(&e, &rule_id),
        e,
    }
}

fn err(code: WeightRuleError) -> Result<Error, InvokeError> {
    Ok(Error::from_contract_error(code as u32))
}

/// A member holding `tokens` tokens, with a live grant to themselves.
fn active_member(f: &Fixture, tokens: u32) -> Address {
    let account = Address::generate(&f.e);
    for _ in 0..tokens {
        f.membership.issue(&account);
    }
    f.membership.delegate_for(&account, &account, &TERM);
    account
}

// ################## ONE TOKEN ONE VOTE ##################

#[test]
fn one_token_one_vote_weighs_the_tokens() {
    let f = setup(WeightModel::OneTokenOneVote);
    let alice = active_member(&f, 3);
    let bob = active_member(&f, 1);

    let now = f.e.ledger().sequence();
    assert_eq!(f.rule.weight_at(&alice, &now), 3);
    assert_eq!(f.rule.weight_at(&bob, &now), 1);
    assert_eq!(f.rule.total_weight_at(&now), 4);
}

#[test]
fn a_member_who_has_not_granted_has_no_weight() {
    let f = setup(WeightModel::OneTokenOneVote);
    let alice = Address::generate(&f.e);
    f.membership.issue(&alice);

    let now = f.e.ledger().sequence();
    assert_eq!(f.rule.weight_at(&alice, &now), 0);
    assert_eq!(f.rule.total_weight_at(&now), 0);
}

#[test]
fn weight_follows_a_grant_to_the_delegate() {
    let f = setup(WeightModel::OneTokenOneVote);
    let alice = active_member(&f, 2);
    let dele = active_member(&f, 1);

    f.membership.delegate_for(&alice, &dele, &TERM);

    let now = f.e.ledger().sequence();
    assert_eq!(f.rule.weight_at(&alice, &now), 0);
    assert_eq!(f.rule.weight_at(&dele, &now), 3);
    assert_eq!(f.rule.total_weight_at(&now), 3);
}

// ################## ONE MEMBER ONE VOTE ##################

#[test]
fn one_member_one_vote_ignores_how_many_tokens_are_held() {
    let f = setup(WeightModel::OneMemberOneVote);
    let alice = active_member(&f, 10);
    let bob = active_member(&f, 1);

    let now = f.e.ledger().sequence();
    assert_eq!(f.rule.weight_at(&alice, &now), 1);
    assert_eq!(f.rule.weight_at(&bob, &now), 1);
    assert_eq!(f.rule.total_weight_at(&now), 2);
}

#[test]
fn one_member_one_vote_gives_a_delegate_a_vote_per_member() {
    let f = setup(WeightModel::OneMemberOneVote);
    let alice = active_member(&f, 10);
    let bob = active_member(&f, 4);
    let dele = Address::generate(&f.e);

    f.membership.delegate_for(&alice, &dele, &TERM);
    f.membership.delegate_for(&bob, &dele, &TERM);

    let now = f.e.ledger().sequence();
    assert_eq!(f.rule.weight_at(&dele, &now), 2);
    assert_eq!(f.rule.total_weight_at(&now), 2);
}

// ################## TIME WEIGHTED ##################

#[test]
fn time_weighting_rewards_unbroken_membership_in_steps() {
    let f = setup_with(
        WeightModel::TimeWeighted,
        Some(TenureSchedule {
            step_ledgers: STEP,
            max_steps: 5,
        }),
    );
    let alice = active_member(&f, 1);

    assert_eq!(f.rule.weight_at(&alice, &START), 1);
    assert_eq!(f.rule.weight_at(&alice, &(START + STEP - 1)), 1);
    assert_eq!(f.rule.weight_at(&alice, &(START + STEP)), 2);
    assert_eq!(f.rule.weight_at(&alice, &(START + 3 * STEP)), 4);
}

#[test]
fn time_weighting_stops_at_the_ceiling() {
    let f = setup_with(
        WeightModel::TimeWeighted,
        Some(TenureSchedule {
            step_ledgers: STEP,
            max_steps: 5,
        }),
    );
    let alice = active_member(&f, 2);

    // Five steps is the ceiling, so six times the power and no more.
    assert_eq!(f.rule.weight_at(&alice, &(START + 5 * STEP)), 12);
    assert_eq!(f.rule.weight_at(&alice, &(START + 50 * STEP)), 12);
}

#[test]
fn time_weighting_gives_nothing_to_an_account_with_no_power() {
    let f = setup_with(
        WeightModel::TimeWeighted,
        Some(TenureSchedule {
            step_ledgers: STEP,
            max_steps: 5,
        }),
    );
    let alice = Address::generate(&f.e);
    f.membership.issue(&alice);

    // Tenure without a live grant is still nothing. Turning up is the
    // condition, seniority only multiplies it.
    assert_eq!(f.rule.weight_at(&alice, &(START + 10 * STEP)), 0);
}

/// Deploying a time weighted rule with the given schedule, for the tests that
/// check the constructor refuses bad ones.
fn deploy_time_weighted(tenure: Option<TenureSchedule>) {
    let e = Env::default();
    let membership = Address::generate(&e);
    e.register(
        RatifyWeightRule,
        (membership, WeightModel::TimeWeighted, tenure),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn time_weighting_needs_a_schedule() {
    deploy_time_weighted(None);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn a_step_of_no_ledgers_is_refused() {
    deploy_time_weighted(Some(TenureSchedule {
        step_ledgers: 0,
        max_steps: 5,
    }));
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn a_schedule_with_no_steps_is_refused() {
    deploy_time_weighted(Some(TenureSchedule {
        step_ledgers: STEP,
        max_steps: 0,
    }));
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn a_schedule_past_the_ceiling_is_refused() {
    deploy_time_weighted(Some(TenureSchedule {
        step_ledgers: STEP,
        max_steps: MAX_TENURE_STEPS + 1,
    }));
}

// ################## WHAT THE GOVERNOR SEES ##################

#[test]
fn the_governor_reads_weight_through_the_standard_votes_interface() {
    let f = setup(WeightModel::OneMemberOneVote);
    let alice = active_member(&f, 10);

    let now = f.e.ledger().sequence();
    assert_eq!(f.votes.get_votes(&alice), 1);
    assert_eq!(f.votes.get_votes_at_checkpoint(&alice, &now), 1);
    assert_eq!(f.votes.get_total_supply(), 1);
    assert_eq!(f.votes.get_total_supply_at_checkpoint(&now), 1);
    assert_eq!(f.votes.get_delegate(&alice), Some(alice));
}

#[test]
fn a_lapsed_grant_is_worth_nothing_to_the_governor() {
    let f = setup(WeightModel::OneTokenOneVote);
    let alice = active_member(&f, 3);

    f.e.ledger().set_sequence_number(START + TERM);
    f.membership.lapse(&alice);

    let now = f.e.ledger().sequence();
    assert_eq!(f.votes.get_votes_at_checkpoint(&alice, &now), 0);
    assert_eq!(f.votes.get_total_supply_at_checkpoint(&now), 0);

    // The past is untouched, so a proposal opened while the grant was live is
    // still decided on the weight that existed then.
    assert_eq!(f.votes.get_votes_at_checkpoint(&alice, &(START + 1)), 3);
}

#[test]
fn delegation_cannot_be_made_here() {
    let f = setup(WeightModel::OneTokenOneVote);
    let alice = active_member(&f, 1);
    let dele = Address::generate(&f.e);

    assert_eq!(
        f.votes.try_delegate(&alice, &dele),
        Err(err(WeightRuleError::DelegateOnMembership))
    );
}

#[test]
fn the_model_is_readable_and_fixed() {
    let f = setup(WeightModel::OneMemberOneVote);
    assert_eq!(f.rule.model(), WeightModel::OneMemberOneVote);
}
