#![cfg(test)]
extern crate std;

use soroban_sdk::{
    contract, contractimpl, symbol_short,
    testutils::{Address as _, Ledger as _},
    Address, BytesN, Env, Error, InvokeError, Map,
};

use crate::{
    RatifyDelegateRegistry, RatifyDelegateRegistryClient, Ballot, RegistryError, VoteCounts,
};

const START: u32 = 10_000;
const DEADLINE: u32 = 10_500;
/// Contested means decided by less than a fifth of the votes cast.
const MARGIN_BPS: u32 = 2_000;

/// Stands in for the governor and the weight rule, which is all the registry
/// reads from them: final tallies, and what an account's vote was worth at a
/// ledger.
#[contract]
pub struct Stub;

#[contractimpl]
impl Stub {
    pub fn set_counts(e: &Env, for_votes: u128, against_votes: u128) {
        e.storage().instance().set(
            &symbol_short!("counts"),
            &VoteCounts { for_votes, against_votes, abstain_votes: 0 },
        );
    }

    pub fn get_proposal_vote_counts(e: &Env, _proposal_id: BytesN<32>) -> VoteCounts {
        e.storage().instance().get(&symbol_short!("counts")).unwrap_or(VoteCounts {
            against_votes: 0,
            for_votes: 0,
            abstain_votes: 0,
        })
    }

    pub fn set_weight(e: &Env, account: Address, weight: u128) {
        let mut weights: Map<Address, u128> = e
            .storage()
            .instance()
            .get(&symbol_short!("weights"))
            .unwrap_or(Map::new(e));
        weights.set(account, weight);
        e.storage().instance().set(&symbol_short!("weights"), &weights);
    }

    pub fn weight_at(e: &Env, account: Address, _ledger: u32) -> u128 {
        let weights: Map<Address, u128> = e
            .storage()
            .instance()
            .get(&symbol_short!("weights"))
            .unwrap_or(Map::new(e));
        weights.get(account).unwrap_or(0)
    }
}

struct Fixture<'a> {
    e: Env,
    stub: StubClient<'a>,
    stub_id: Address,
    registry: RatifyDelegateRegistryClient<'a>,
}

fn setup<'a>() -> Fixture<'a> {
    let e = Env::default();
    e.mock_all_auths();
    e.ledger().set_sequence_number(START);

    let stub_id = e.register(Stub, ());
    let registry_id = e.register(
        RatifyDelegateRegistry,
        (stub_id.clone(), stub_id.clone(), MARGIN_BPS),
    );

    Fixture {
        stub: StubClient::new(&e, &stub_id),
        registry: RatifyDelegateRegistryClient::new(&e, &registry_id),
        stub_id,
        e,
    }
}

fn err(code: RegistryError) -> Result<Error, InvokeError> {
    Ok(Error::from_contract_error(code as u32))
}

fn pid(e: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(e, &[byte; 32])
}

/// A proposal that is open for voting.
fn proposal(f: &Fixture, byte: u8) -> BytesN<32> {
    let id = pid(&f.e, byte);
    f.registry.open(&id, &START, &DEADLINE);
    id
}

fn close(f: &Fixture) {
    f.e.ledger().set_sequence_number(DEADLINE + 1);
}

// ################## WHO MAY WRITE ##################

#[test]
fn the_registry_knows_its_governor() {
    let f = setup();
    assert_eq!(f.registry.governor(), f.stub_id);
    assert_eq!(f.registry.contested_margin_bps(), MARGIN_BPS);
}

#[test]
fn only_the_governor_may_open_a_proposal_or_record_a_vote() {
    let f = setup();
    let id = pid(&f.e, 1);

    f.e.set_auths(&[]);
    assert!(f.registry.try_open(&id, &START, &DEADLINE).is_err());

    let alice = Address::generate(&f.e);
    assert!(f
        .registry
        .try_record_vote(&alice, &id, &Ballot::For, &1)
        .is_err());
}

#[test]
fn a_vote_on_an_unknown_proposal_is_refused() {
    let f = setup();
    let alice = Address::generate(&f.e);
    assert_eq!(
        f.registry.try_record_vote(&alice, &pid(&f.e, 9), &Ballot::For, &1),
        Err(err(RegistryError::ProposalNotFound))
    );
}

#[test]
fn a_proposal_cannot_be_opened_twice() {
    let f = setup();
    let id = proposal(&f, 1);
    assert_eq!(
        f.registry.try_open(&id, &START, &DEADLINE),
        Err(err(RegistryError::ProposalAlreadyOpen))
    );
}

// ################## TURNING UP, AND NOT ##################

#[test]
fn a_delegate_who_votes_gets_credit_for_it() {
    let f = setup();
    let id = proposal(&f, 1);
    let dele = Address::generate(&f.e);
    f.stub.set_weight(&dele, &5);
    f.stub.set_counts(&100, &10);

    f.registry.record_vote(&dele, &id, &Ballot::For, &5);
    close(&f);
    f.registry.settle(&dele, &id);

    let record = f.registry.record(&dele);
    assert_eq!(record.eligible, 1);
    assert_eq!(record.voted, 1);
    assert_eq!(f.registry.participation_bps(&dele), Some(10_000));
}

#[test]
fn a_delegate_who_stays_away_is_recorded_as_having_stayed_away() {
    let f = setup();
    let id = proposal(&f, 1);
    let dele = Address::generate(&f.e);
    f.stub.set_weight(&dele, &5);
    f.stub.set_counts(&100, &10);

    close(&f);

    f.e.set_auths(&[]);
    f.registry.settle(&dele, &id);

    let record = f.registry.record(&dele);
    assert_eq!(record.eligible, 1);
    assert_eq!(record.voted, 0);
    assert_eq!(f.registry.participation_bps(&dele), Some(0));
}

#[test]
fn a_participation_rate_out_of_nothing_is_unknown_not_zero() {
    let f = setup();
    let newcomer = Address::generate(&f.e);
    assert_eq!(f.registry.participation_bps(&newcomer), None);
    assert_eq!(f.registry.contested_participation_bps(&newcomer), None);
}

#[test]
fn a_rate_is_the_share_of_proposals_turned_up_for() {
    let f = setup();
    let dele = Address::generate(&f.e);
    f.stub.set_weight(&dele, &5);
    f.stub.set_counts(&100, &10);

    let first = proposal(&f, 1);
    let second = proposal(&f, 2);
    let third = proposal(&f, 3);
    let fourth = proposal(&f, 4);
    f.registry.record_vote(&dele, &first, &Ballot::For, &5);
    f.registry.record_vote(&dele, &third, &Ballot::Against, &5);

    close(&f);
    for id in [first, second, third, fourth] {
        f.registry.settle(&dele, &id);
    }

    let record = f.registry.record(&dele);
    assert_eq!(record.eligible, 4);
    assert_eq!(record.voted, 2);
    assert_eq!(f.registry.participation_bps(&dele), Some(5_000));
}

// ################## WHO WAS ELIGIBLE ##################

#[test]
fn an_account_with_no_power_at_the_snapshot_was_never_eligible() {
    let f = setup();
    let id = proposal(&f, 1);
    let stranger = Address::generate(&f.e);
    f.stub.set_counts(&100, &10);
    close(&f);

    assert_eq!(
        f.registry.try_settle(&stranger, &id),
        Err(err(RegistryError::NotEligible))
    );
    assert_eq!(f.registry.record(&stranger).eligible, 0);
}

#[test]
fn nothing_can_be_settled_while_voting_is_open() {
    let f = setup();
    let id = proposal(&f, 1);
    let dele = Address::generate(&f.e);
    f.stub.set_weight(&dele, &5);

    f.e.ledger().set_sequence_number(DEADLINE);
    assert_eq!(
        f.registry.try_settle(&dele, &id),
        Err(err(RegistryError::VotingStillOpen))
    );
}

#[test]
fn settling_twice_changes_nothing() {
    let f = setup();
    let id = proposal(&f, 1);
    let dele = Address::generate(&f.e);
    f.stub.set_weight(&dele, &5);
    f.stub.set_counts(&100, &10);
    close(&f);

    f.registry.settle(&dele, &id);
    assert_eq!(
        f.registry.try_settle(&dele, &id),
        Err(err(RegistryError::AlreadySettled))
    );
    assert_eq!(f.registry.record(&dele).eligible, 1);
}

// ################## CONTESTED PROPOSALS ##################

#[test]
fn a_close_result_is_marked_contested() {
    let f = setup();
    let id = proposal(&f, 1);
    let dele = Address::generate(&f.e);
    f.stub.set_weight(&dele, &5);

    f.stub.set_counts(&52, &48);
    f.registry.record_vote(&dele, &id, &Ballot::For, &5);
    close(&f);
    f.registry.settle(&dele, &id);

    assert!(f.registry.proposal(&id).unwrap().contested);
    let record = f.registry.record(&dele);
    assert_eq!(record.contested_eligible, 1);
    assert_eq!(record.contested_voted, 1);
    assert_eq!(record.contested_with_outcome, 1);
    assert_eq!(f.registry.contested_participation_bps(&dele), Some(10_000));
}

#[test]
fn a_landslide_is_not_contested() {
    let f = setup();
    let id = proposal(&f, 1);
    let dele = Address::generate(&f.e);
    f.stub.set_weight(&dele, &5);

    f.stub.set_counts(&95, &5);
    close(&f);
    f.registry.settle(&dele, &id);

    assert!(!f.registry.proposal(&id).unwrap().contested);
    let record = f.registry.record(&dele);
    assert_eq!(record.eligible, 1);
    assert_eq!(record.contested_eligible, 0);
    assert_eq!(f.registry.contested_participation_bps(&dele), None);
}

#[test]
fn missing_the_close_ones_shows_up_separately() {
    let f = setup();
    let dele = Address::generate(&f.e);
    f.stub.set_weight(&dele, &5);

    let easy = proposal(&f, 1);
    f.registry.record_vote(&dele, &easy, &Ballot::For, &5);
    f.stub.set_counts(&95, &5);
    close(&f);
    f.registry.settle(&dele, &easy);

    f.e.ledger().set_sequence_number(START);
    let close_one = proposal(&f, 2);
    f.stub.set_counts(&51, &49);
    close(&f);
    f.registry.settle(&dele, &close_one);

    let record = f.registry.record(&dele);
    assert_eq!(f.registry.participation_bps(&dele), Some(5_000));
    assert_eq!(f.registry.contested_participation_bps(&dele), Some(0));
    assert_eq!(record.contested_eligible, 1);
    assert_eq!(record.contested_voted, 0);
}

#[test]
fn voting_against_the_outcome_still_counts_as_turning_up() {
    let f = setup();
    let id = proposal(&f, 1);
    let dele = Address::generate(&f.e);
    f.stub.set_weight(&dele, &5);

    f.stub.set_counts(&51, &49);
    f.registry.record_vote(&dele, &id, &Ballot::Against, &5);
    close(&f);
    f.registry.settle(&dele, &id);

    let record = f.registry.record(&dele);
    assert_eq!(record.contested_voted, 1);
    assert_eq!(record.contested_with_outcome, 0);
}

#[test]
fn an_abstention_is_attendance_without_a_side() {
    let f = setup();
    let id = proposal(&f, 1);
    let dele = Address::generate(&f.e);
    f.stub.set_weight(&dele, &5);

    f.stub.set_counts(&51, &49);
    f.registry.record_vote(&dele, &id, &Ballot::Abstain, &5);
    close(&f);
    f.registry.settle(&dele, &id);

    let record = f.registry.record(&dele);
    assert_eq!(record.voted, 1);
    assert_eq!(record.contested_voted, 1);
    assert_eq!(record.contested_with_outcome, 0);
}

#[test]
fn a_proposal_nobody_voted_on_is_not_contested() {
    let f = setup();
    let id = proposal(&f, 1);
    let dele = Address::generate(&f.e);
    f.stub.set_weight(&dele, &5);

    f.stub.set_counts(&0, &0);
    close(&f);
    f.registry.settle(&dele, &id);

    assert!(!f.registry.proposal(&id).unwrap().contested);
}

#[test]
fn every_account_settled_on_a_proposal_is_judged_on_the_same_tallies() {
    let f = setup();
    let id = proposal(&f, 1);
    let first = Address::generate(&f.e);
    let second = Address::generate(&f.e);
    f.stub.set_weight(&first, &5);
    f.stub.set_weight(&second, &5);

    f.stub.set_counts(&51, &49);
    close(&f);
    f.registry.settle(&first, &id);

    f.stub.set_counts(&99, &1);
    f.registry.settle(&second, &id);

    assert_eq!(f.registry.record(&first).contested_eligible, 1);
    assert_eq!(f.registry.record(&second).contested_eligible, 1);
}

#[test]
fn the_record_says_how_current_it_is() {
    let f = setup();
    let id = proposal(&f, 1);
    let dele = Address::generate(&f.e);
    f.stub.set_weight(&dele, &5);
    f.stub.set_counts(&60, &40);
    close(&f);

    f.registry.settle(&dele, &id);

    assert_eq!(f.registry.record(&dele).last_settled_deadline, DEADLINE);
    assert!(f.registry.is_settled(&id, &dele));
    assert_eq!(f.registry.ballot(&id, &dele), None);
}
