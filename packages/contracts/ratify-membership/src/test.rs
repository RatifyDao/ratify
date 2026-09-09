#![cfg(test)]
extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, Error, InvokeError, String,
};

use crate::{RatifyMembership, RatifyMembershipClient, MembershipError, MAX_TERM_LEDGERS};

const START: u32 = 10_000;
const TERM: u32 = 5_000;

struct Fixture<'a> {
    e: Env,
    issuer: Address,
    membership: RatifyMembershipClient<'a>,
}

fn setup<'a>() -> Fixture<'a> {
    let e = Env::default();
    e.mock_all_auths();
    e.ledger().set_sequence_number(START);

    let issuer = Address::generate(&e);
    let id = e.register(
        RatifyMembership,
        (
            issuer.clone(),
            String::from_str(&e, "Riverside Commons"),
            String::from_str(&e, "RIVER"),
            String::from_str(&e, "https://example.invalid/member/"),
        ),
    );

    Fixture { membership: RatifyMembershipClient::new(&e, &id), e, issuer }
}

fn err(code: MembershipError) -> Result<Error, InvokeError> {
    Ok(Error::from_contract_error(code as u32))
}

/// A member holding one token, with no voting power yet.
fn member(f: &Fixture) -> Address {
    let account = Address::generate(&f.e);
    f.membership.issue(&account);
    account
}

// ################## THE CREDENTIAL ##################

#[test]
fn a_new_community_has_no_members_and_no_power() {
    let f = setup();
    assert_eq!(f.membership.member_count(), 0);
    assert_eq!(f.membership.live_total(), 0);
    assert_eq!(f.membership.issuer(), f.issuer);
    assert_eq!(f.membership.name(), String::from_str(&f.e, "Riverside Commons"));
}

#[test]
fn issuing_a_token_admits_a_member_but_grants_no_power() {
    let f = setup();
    let alice = member(&f);

    assert_eq!(f.membership.balance(&alice), 1);
    assert_eq!(f.membership.member_count(), 1);

    // The token is a credential. Turning up is a separate act.
    assert_eq!(f.membership.votes(&alice), 0);
    assert_eq!(f.membership.live_total(), 0);
    assert_eq!(f.membership.grant(&alice), None);
    assert!(!f.membership.is_live(&alice));
}

#[test]
fn a_member_holding_several_tokens_is_counted_once() {
    let f = setup();
    let alice = member(&f);
    f.membership.issue(&alice);
    f.membership.issue(&alice);

    assert_eq!(f.membership.balance(&alice), 3);
    assert_eq!(f.membership.member_count(), 1);
}

#[test]
fn only_the_issuer_may_admit_or_remove_a_member() {
    let f = setup();
    let alice = Address::generate(&f.e);

    f.e.set_auths(&[]);
    assert!(f.membership.try_issue(&alice).is_err());
    assert_eq!(f.membership.member_count(), 0);
}

#[test]
fn issuance_can_be_moved_under_governance() {
    let f = setup();
    let timelock = Address::generate(&f.e);

    f.membership.transfer_issuance(&timelock);

    assert_eq!(f.membership.issuer(), timelock);
}

#[test]
fn revoking_removes_the_member() {
    let f = setup();
    let alice = member(&f);
    let token = 0u32;

    f.membership.revoke(&token);

    assert_eq!(f.membership.balance(&alice), 0);
    assert_eq!(f.membership.member_count(), 0);
}

// ################## GRANTING POWER ##################

#[test]
fn a_member_grants_power_to_themselves_for_a_term() {
    let f = setup();
    let alice = member(&f);

    f.membership.delegate_for(&alice, &alice, &TERM);

    assert_eq!(f.membership.votes(&alice), 1);
    assert_eq!(f.membership.live_total(), 1);
    assert!(f.membership.is_live(&alice));
    assert_eq!(f.membership.ledgers_until_lapse(&alice), TERM);

    let grant = f.membership.grant(&alice).unwrap();
    assert_eq!(grant.delegatee, alice);
    assert_eq!(grant.units, 1);
    assert_eq!(grant.expires_at, START + TERM);
    assert_eq!(grant.granted_at, START);
}

#[test]
fn a_member_can_hand_their_vote_to_a_delegate() {
    let f = setup();
    let alice = member(&f);
    let dele = member(&f);

    f.membership.delegate_for(&alice, &dele, &TERM);
    f.membership.delegate_for(&dele, &dele, &TERM);

    assert_eq!(f.membership.votes(&dele), 2);
    assert_eq!(f.membership.votes(&alice), 0);
    assert_eq!(f.membership.live_total(), 2);
}

#[test]
fn a_member_with_no_tokens_has_nothing_to_grant() {
    let f = setup();
    let stranger = Address::generate(&f.e);

    assert_eq!(
        f.membership.try_delegate_for(&stranger, &stranger, &TERM),
        Err(err(MembershipError::NoVotingUnits))
    );
}

#[test]
fn a_grant_must_carry_a_usable_term() {
    let f = setup();
    let alice = member(&f);

    assert_eq!(
        f.membership.try_delegate_for(&alice, &alice, &0),
        Err(err(MembershipError::TermCannotBeZero))
    );
    assert_eq!(
        f.membership.try_delegate_for(&alice, &alice, &(MAX_TERM_LEDGERS + 1)),
        Err(err(MembershipError::TermTooLong))
    );
}

#[test]
fn granting_again_moves_the_power_at_once() {
    let f = setup();
    let alice = member(&f);
    let first = Address::generate(&f.e);
    let second = Address::generate(&f.e);

    f.membership.delegate_for(&alice, &first, &TERM);
    assert_eq!(f.membership.votes(&first), 1);

    f.membership.delegate_for(&alice, &second, &TERM);

    assert_eq!(f.membership.votes(&first), 0);
    assert_eq!(f.membership.votes(&second), 1);
    assert_eq!(f.membership.live_total(), 1);
}

#[test]
fn a_grant_requires_the_member_s_own_authorisation() {
    let f = setup();
    let alice = member(&f);
    let thief = Address::generate(&f.e);

    f.e.set_auths(&[]);
    assert!(f.membership.try_delegate_for(&alice, &thief, &TERM).is_err());
    assert_eq!(f.membership.votes(&thief), 0);
}

// ################## LAPSING ##################

#[test]
fn a_grant_stops_being_live_when_its_term_ends() {
    let f = setup();
    let alice = member(&f);
    f.membership.delegate_for(&alice, &alice, &TERM);

    f.e.ledger().set_sequence_number(START + TERM - 1);
    assert!(f.membership.is_live(&alice));
    assert_eq!(f.membership.ledgers_until_lapse(&alice), 1);

    f.e.ledger().set_sequence_number(START + TERM);
    assert!(!f.membership.is_live(&alice));
    assert_eq!(f.membership.ledgers_until_lapse(&alice), 0);
}

#[test]
fn anyone_may_sweep_an_expired_grant() {
    let f = setup();
    let alice = member(&f);
    f.membership.delegate_for(&alice, &alice, &TERM);
    assert_eq!(f.membership.live_total(), 1);

    f.e.ledger().set_sequence_number(START + TERM);

    // No authorisation of any kind. The member consented to this when they
    // chose a term.
    f.e.set_auths(&[]);
    f.membership.lapse(&alice);

    assert_eq!(f.membership.votes(&alice), 0);
    assert_eq!(f.membership.live_total(), 0);
    assert_eq!(f.membership.grant(&alice), None);
}

#[test]
fn a_live_grant_cannot_be_swept() {
    let f = setup();
    let alice = member(&f);
    f.membership.delegate_for(&alice, &alice, &TERM);

    f.e.ledger().set_sequence_number(START + TERM - 1);
    assert_eq!(f.membership.try_lapse(&alice), Err(err(MembershipError::GrantStillLive)));
    assert_eq!(f.membership.live_total(), 1);
}

#[test]
fn sweeping_a_delegate_s_lapsed_grant_costs_them_the_weight() {
    let f = setup();
    let alice = member(&f);
    let bob = member(&f);
    let dele = Address::generate(&f.e);

    f.membership.delegate_for(&alice, &dele, &TERM);
    f.membership.delegate_for(&bob, &dele, &(TERM * 2));
    assert_eq!(f.membership.votes(&dele), 2);

    f.e.ledger().set_sequence_number(START + TERM);
    f.membership.lapse(&alice);

    // Bob's grant runs longer, so the delegate keeps that half of it.
    assert_eq!(f.membership.votes(&dele), 1);
    assert_eq!(f.membership.live_total(), 1);
}

#[test]
fn a_member_can_take_their_grant_back_early() {
    let f = setup();
    let alice = member(&f);
    let dele = Address::generate(&f.e);
    f.membership.delegate_for(&alice, &dele, &TERM);

    f.membership.withdraw(&alice);

    assert_eq!(f.membership.votes(&dele), 0);
    assert_eq!(f.membership.live_total(), 0);
    assert_eq!(f.membership.grant(&alice), None);
}

#[test]
fn renewing_extends_a_term_without_changing_who_holds_it() {
    let f = setup();
    let alice = member(&f);
    let dele = Address::generate(&f.e);
    f.membership.delegate_for(&alice, &dele, &TERM);

    f.e.ledger().set_sequence_number(START + TERM - 10);
    f.membership.renew(&alice, &TERM);

    let grant = f.membership.grant(&alice).unwrap();
    assert_eq!(grant.delegatee, dele);
    assert_eq!(grant.expires_at, START + TERM - 10 + TERM);
    assert_eq!(f.membership.votes(&dele), 1);
    assert_eq!(f.membership.live_total(), 1);
}

#[test]
fn a_grant_that_has_already_lapsed_cannot_be_renewed() {
    let f = setup();
    let alice = member(&f);
    f.membership.delegate_for(&alice, &alice, &TERM);

    f.e.ledger().set_sequence_number(START + TERM);
    assert_eq!(f.membership.try_renew(&alice, &TERM), Err(err(MembershipError::NoGrant)));
}

// ################## POWER AT A PAST LEDGER ##################

#[test]
fn power_is_read_as_it_stood_at_a_past_ledger() {
    let f = setup();
    let alice = member(&f);

    f.e.ledger().set_sequence_number(START + 100);
    f.membership.delegate_for(&alice, &alice, &TERM);

    f.e.ledger().set_sequence_number(START + 200);
    f.membership.issue(&alice);

    f.e.ledger().set_sequence_number(START + 300);

    assert_eq!(f.membership.votes_at(&alice, &(START + 50)), 0);
    assert_eq!(f.membership.votes_at(&alice, &(START + 150)), 1);
    assert_eq!(f.membership.votes_at(&alice, &(START + 250)), 2);
    assert_eq!(f.membership.votes(&alice), 2);
}

#[test]
fn a_lapse_does_not_rewrite_the_past() {
    let f = setup();
    let alice = member(&f);
    f.membership.delegate_for(&alice, &alice, &TERM);

    f.e.ledger().set_sequence_number(START + TERM + 100);
    f.membership.lapse(&alice);

    // A proposal that opened while the grant was live is still decided on the
    // power that existed then.
    assert_eq!(f.membership.votes_at(&alice, &(START + 10)), 1);
    assert_eq!(f.membership.live_total_at(&(START + 10)), 1);
    assert_eq!(f.membership.live_total(), 0);
}

#[test]
fn several_changes_in_one_ledger_leave_one_reading() {
    let f = setup();
    let alice = member(&f);
    let bob = member(&f);

    f.membership.delegate_for(&alice, &alice, &TERM);
    f.membership.delegate_for(&bob, &bob, &TERM);
    f.membership.withdraw(&bob);

    assert_eq!(f.membership.live_total(), 1);
    assert_eq!(f.membership.live_total_at(&START), 1);
}

// ################## TOKENS ARRIVING AND LEAVING ##################

#[test]
fn a_token_issued_to_a_member_with_a_live_grant_adds_to_their_delegate() {
    let f = setup();
    let alice = member(&f);
    let dele = Address::generate(&f.e);
    f.membership.delegate_for(&alice, &dele, &TERM);
    assert_eq!(f.membership.votes(&dele), 1);

    f.membership.issue(&alice);

    assert_eq!(f.membership.votes(&dele), 2);
    assert_eq!(f.membership.live_total(), 2);
    assert_eq!(f.membership.grant(&alice).unwrap().units, 2);
}

#[test]
fn revoking_a_token_takes_the_power_with_it() {
    let f = setup();
    let alice = member(&f);
    f.membership.issue(&alice);
    let dele = Address::generate(&f.e);
    f.membership.delegate_for(&alice, &dele, &TERM);
    assert_eq!(f.membership.votes(&dele), 2);

    f.membership.revoke(&0);

    assert_eq!(f.membership.votes(&dele), 1);
    assert_eq!(f.membership.live_total(), 1);
    assert_eq!(f.membership.member_count(), 1);
}

#[test]
fn issuing_to_a_member_whose_grant_lapsed_sweeps_it_instead() {
    let f = setup();
    let alice = member(&f);
    f.membership.delegate_for(&alice, &alice, &TERM);

    f.e.ledger().set_sequence_number(START + TERM);
    f.membership.issue(&alice);

    // The expired grant is swept rather than grown, so a member who stopped
    // renewing cannot add weight to a delegate by acquiring another token.
    assert_eq!(f.membership.live_total(), 0);
    assert_eq!(f.membership.votes(&alice), 0);
    assert_eq!(f.membership.grant(&alice), None);

    f.membership.delegate_for(&alice, &alice, &TERM);
    assert_eq!(f.membership.votes(&alice), 2);
    assert_eq!(f.membership.live_total(), 2);
}

#[test]
fn re_granting_after_a_lapse_does_not_double_count() {
    let f = setup();
    let alice = member(&f);
    let dele = Address::generate(&f.e);
    f.membership.delegate_for(&alice, &dele, &TERM);

    // The term ends and nobody sweeps it.
    f.e.ledger().set_sequence_number(START + TERM);
    assert_eq!(f.membership.live_total(), 1);

    // Alice grants again. The stale power has to come off the old delegate
    // even though the grant was never swept.
    f.membership.delegate_for(&alice, &alice, &TERM);

    assert_eq!(f.membership.votes(&dele), 0);
    assert_eq!(f.membership.votes(&alice), 1);
    assert_eq!(f.membership.live_total(), 1);
}

#[test]
fn withdrawing_a_lapsed_grant_does_not_double_count() {
    let f = setup();
    let alice = member(&f);
    f.membership.delegate_for(&alice, &alice, &TERM);

    f.e.ledger().set_sequence_number(START + TERM);
    f.membership.withdraw(&alice);

    assert_eq!(f.membership.live_total(), 0);
    assert_eq!(f.membership.votes(&alice), 0);
}

// ################## HEADS AND TENURE ##################

#[test]
fn one_member_one_vote_counts_members_not_tokens() {
    let f = setup();
    let alice = member(&f);
    f.membership.issue(&alice);
    f.membership.issue(&alice);
    let bob = member(&f);

    f.membership.delegate_for(&alice, &alice, &TERM);
    f.membership.delegate_for(&bob, &bob, &TERM);

    // Alice holds three tokens and Bob one, but they are one member each.
    assert_eq!(f.membership.votes(&alice), 3);
    assert_eq!(f.membership.heads(&alice), 1);
    assert_eq!(f.membership.heads(&bob), 1);
    assert_eq!(f.membership.live_heads(), 2);
    assert_eq!(f.membership.live_total(), 4);
}

#[test]
fn a_delegate_gathers_a_head_per_member_who_grants_to_them() {
    let f = setup();
    let alice = member(&f);
    let bob = member(&f);
    let dele = Address::generate(&f.e);

    f.membership.delegate_for(&alice, &dele, &TERM);
    f.membership.delegate_for(&bob, &dele, &TERM);

    assert_eq!(f.membership.heads(&dele), 2);
    assert_eq!(f.membership.live_heads(), 2);

    f.e.ledger().set_sequence_number(START + TERM);
    f.membership.lapse(&alice);

    assert_eq!(f.membership.heads(&dele), 1);
    assert_eq!(f.membership.live_heads(), 1);
}

#[test]
fn a_further_token_does_not_add_a_head() {
    let f = setup();
    let alice = member(&f);
    f.membership.delegate_for(&alice, &alice, &TERM);

    f.membership.issue(&alice);

    assert_eq!(f.membership.votes(&alice), 2);
    assert_eq!(f.membership.heads(&alice), 1);
    assert_eq!(f.membership.live_heads(), 1);
}

#[test]
fn heads_are_read_as_they_stood_at_a_past_ledger() {
    let f = setup();
    let alice = member(&f);
    let bob = member(&f);
    let dele = Address::generate(&f.e);

    f.membership.delegate_for(&alice, &dele, &TERM);
    f.e.ledger().set_sequence_number(START + 100);
    f.membership.delegate_for(&bob, &dele, &TERM);

    assert_eq!(f.membership.heads_at(&dele, &(START + 50)), 1);
    assert_eq!(f.membership.heads_at(&dele, &(START + 150)), 2);
    assert_eq!(f.membership.live_heads_at(&(START - 1)), 0);
}

#[test]
fn tenure_runs_from_the_first_token_and_survives_the_rest() {
    let f = setup();
    let alice = member(&f);
    assert_eq!(f.membership.member_since(&alice), Some(START));

    f.e.ledger().set_sequence_number(START + 1_000);
    f.membership.issue(&alice);

    // A second token does not reset the clock.
    assert_eq!(f.membership.member_since(&alice), Some(START));
    assert_eq!(f.membership.tenure_at(&alice, &(START + 1_000)), 1_000);

    // Nor does shedding one, while any remain.
    f.membership.revoke(&0);
    assert_eq!(f.membership.member_since(&alice), Some(START));
}

#[test]
fn tenure_restarts_for_a_member_who_left_and_returned() {
    let f = setup();
    let alice = member(&f);
    f.membership.revoke(&0);
    assert_eq!(f.membership.member_since(&alice), None);

    f.e.ledger().set_sequence_number(START + 5_000);
    f.membership.issue(&alice);

    assert_eq!(f.membership.member_since(&alice), Some(START + 5_000));
    assert_eq!(f.membership.tenure_at(&alice, &(START + 6_000)), 1_000);
}

#[test]
fn tenure_at_a_ledger_before_joining_is_nothing() {
    let f = setup();
    f.e.ledger().set_sequence_number(START + 1_000);
    let alice = member(&f);

    assert_eq!(f.membership.tenure_at(&alice, &START), 0);
    assert_eq!(f.membership.tenure_at(&alice, &(START + 1_500)), 500);
}
