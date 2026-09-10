#![cfg(test)]
extern crate std;

use soroban_sdk::{
    contract, contractimpl, symbol_short,
    testutils::{Address as _, Ledger as _},
    Address, BytesN, ConversionError, Env, Error, IntoVal, InvokeError, String, Symbol, Val, Vec,
};
use stellar_governance::timelock::{OperationState, TimelockClient};

use crate::{RatifyTimelock, RatifyTimelockClient, RatifyTimelockError};

const DELAY: u32 = 500;
const START: u32 = 10_000;

/// A stand-in for the treasury: something the timelock can call, that records
/// whether the call actually happened.
#[contract]
pub struct Recorder;

#[contractimpl]
impl Recorder {
    pub fn record(e: &Env, caller: Address, amount: i128) {
        caller.require_auth();
        e.storage()
            .instance()
            .set(&symbol_short!("amount"), &amount);
    }

    pub fn recorded(e: &Env) -> i128 {
        e.storage()
            .instance()
            .get(&symbol_short!("amount"))
            .unwrap_or(0)
    }
}

struct Fixture<'a> {
    e: Env,
    governor: Address,
    guardian: Address,
    timelock_id: Address,
    timelock: RatifyTimelockClient<'a>,
    /// The same contract seen through the standard timelock interface, which
    /// is how the governor and an executor reach it.
    iface: TimelockClient<'a>,
    recorder_id: Address,
    recorder: RecorderClient<'a>,
}

fn setup<'a>() -> Fixture<'a> {
    let e = Env::default();
    e.mock_all_auths();
    e.ledger().set_sequence_number(START);

    let governor = Address::generate(&e);
    let guardian = Address::generate(&e);
    let timelock_id = e.register(RatifyTimelock, (governor.clone(), guardian.clone(), DELAY));
    let recorder_id = e.register(Recorder, ());

    Fixture {
        timelock: RatifyTimelockClient::new(&e, &timelock_id),
        iface: TimelockClient::new(&e, &timelock_id),
        recorder: RecorderClient::new(&e, &recorder_id),
        e,
        governor,
        guardian,
        timelock_id,
        recorder_id,
    }
}

fn err(code: RatifyTimelockError) -> Result<Error, InvokeError> {
    Ok(Error::from_contract_error(code as u32))
}

fn zero(e: &Env) -> BytesN<32> {
    BytesN::from_array(e, &[0u8; 32])
}

fn salt(e: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(e, &[byte; 32])
}

/// The action every test schedules: the recorder is told to store an amount.
fn action(f: &Fixture, amount: i128) -> (Address, Symbol, Vec<Val>) {
    let args: Vec<Val> = Vec::from_array(
        &f.e,
        [f.timelock_id.clone().into_val(&f.e), amount.into_val(&f.e)],
    );
    (f.recorder_id.clone(), symbol_short!("record"), args)
}

fn schedule(f: &Fixture, amount: i128, s: u8) -> BytesN<32> {
    let (target, function, args) = action(f, amount);
    f.iface.schedule(
        &target,
        &function,
        &args,
        &zero(&f.e),
        &salt(&f.e, s),
        &DELAY,
        &f.governor,
    )
}

fn execute(f: &Fixture, amount: i128, s: u8) {
    let (target, function, args) = action(f, amount);
    f.iface.execute(
        &target,
        &function,
        &args,
        &zero(&f.e),
        &salt(&f.e, s),
        &None,
    );
}

type TryResult<T> = Result<Result<T, ConversionError>, Result<Error, InvokeError>>;

fn try_execute(f: &Fixture, amount: i128, s: u8) -> TryResult<Val> {
    let (target, function, args) = action(f, amount);
    f.iface.try_execute(
        &target,
        &function,
        &args,
        &zero(&f.e),
        &salt(&f.e, s),
        &None,
    )
}

// ################## SETUP ##################

#[test]
fn constructor_records_the_roles_and_the_delay() {
    let f = setup();
    assert_eq!(f.timelock.governor(), f.governor);
    assert_eq!(f.timelock.guardian(), f.guardian);
    assert_eq!(f.iface.get_min_delay(), DELAY);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn a_timelock_cannot_be_deployed_with_no_delay() {
    let e = Env::default();
    e.mock_all_auths();
    let governor = Address::generate(&e);
    let guardian = Address::generate(&e);
    e.register(RatifyTimelock, (governor, guardian, 0u32));
}

// ################## WHO MAY SCHEDULE ##################

#[test]
fn only_the_governor_may_schedule() {
    let f = setup();
    let stranger = Address::generate(&f.e);
    let (target, function, args) = action(&f, 100);

    let result = f.iface.try_schedule(
        &target,
        &function,
        &args,
        &zero(&f.e),
        &salt(&f.e, 1),
        &DELAY,
        &stranger,
    );
    assert_eq!(result, Err(err(RatifyTimelockError::NotGovernor)));
}

#[test]
fn a_proposal_cannot_shorten_its_own_review_period() {
    let f = setup();
    let (target, function, args) = action(&f, 100);

    let result = f.iface.try_schedule(
        &target,
        &function,
        &args,
        &zero(&f.e),
        &salt(&f.e, 1),
        &(DELAY - 1),
        &f.governor,
    );
    assert!(result.is_err());
}

// ################## THE DELAY ##################

#[test]
fn a_scheduled_action_waits() {
    let f = setup();
    let id = schedule(&f, 750, 1);

    assert_eq!(f.iface.get_operation_state(&id), OperationState::Waiting);
    assert_eq!(f.timelock.ledgers_remaining(&id), DELAY);
    assert_eq!(f.recorder.recorded(), 0);
}

#[test]
fn executing_early_is_impossible() {
    let f = setup();
    schedule(&f, 750, 1);

    f.e.ledger().set_sequence_number(START + DELAY - 1);
    assert!(try_execute(&f, 750, 1).is_err());
    assert_eq!(f.recorder.recorded(), 0);
}

#[test]
fn the_countdown_runs_down_to_the_ledger() {
    let f = setup();
    let id = schedule(&f, 750, 1);

    f.e.ledger().set_sequence_number(START + 100);
    assert_eq!(f.timelock.ledgers_remaining(&id), DELAY - 100);

    f.e.ledger().set_sequence_number(START + DELAY);
    assert_eq!(f.timelock.ledgers_remaining(&id), 0);
    assert_eq!(f.iface.get_operation_state(&id), OperationState::Ready);
}

#[test]
fn anyone_may_execute_once_the_delay_has_passed() {
    let f = setup();
    let id = schedule(&f, 750, 1);
    f.e.ledger().set_sequence_number(START + DELAY);

    // No executor is named. The decision carries itself through.
    execute(&f, 750, 1);

    assert_eq!(f.recorder.recorded(), 750);
    assert_eq!(f.iface.get_operation_state(&id), OperationState::Done);
}

#[test]
fn executing_twice_is_impossible() {
    let f = setup();
    schedule(&f, 750, 1);
    f.e.ledger().set_sequence_number(START + DELAY);
    execute(&f, 750, 1);

    assert!(try_execute(&f, 750, 1).is_err());
    assert_eq!(f.recorder.recorded(), 750);
}

#[test]
fn an_action_that_was_never_scheduled_cannot_be_executed() {
    let f = setup();
    f.e.ledger().set_sequence_number(START + DELAY);

    assert!(try_execute(&f, 750, 1).is_err());
    assert_eq!(f.recorder.recorded(), 0);
}

#[test]
fn changing_the_action_changes_its_identity() {
    let f = setup();
    schedule(&f, 750, 1);
    f.e.ledger().set_sequence_number(START + DELAY);

    // The same salt, one different argument. Nothing about the approved
    // operation covers this call, so it is not executable.
    assert!(try_execute(&f, 751, 1).is_err());
    assert_eq!(f.recorder.recorded(), 0);

    execute(&f, 750, 1);
    assert_eq!(f.recorder.recorded(), 750);
}

// ################## THE BRAKE ##################

#[test]
fn the_guardian_can_stop_a_waiting_action() {
    let f = setup();
    let id = schedule(&f, 750, 1);

    f.timelock.cancel_with_reason(
        &id,
        &f.guardian,
        &String::from_str(&f.e, "The recipient address is wrong."),
    );

    assert_eq!(f.iface.get_operation_state(&id), OperationState::Unset);
    f.e.ledger().set_sequence_number(START + DELAY);
    assert!(try_execute(&f, 750, 1).is_err());
    assert_eq!(f.recorder.recorded(), 0);
}

#[test]
fn nobody_but_the_guardian_can_stop_one() {
    let f = setup();
    let id = schedule(&f, 750, 1);
    let stranger = Address::generate(&f.e);

    let reason = String::from_str(&f.e, "No.");
    assert_eq!(
        f.timelock.try_cancel_with_reason(&id, &stranger, &reason),
        Err(err(RatifyTimelockError::NotGuardian))
    );
    assert_eq!(
        f.timelock.try_cancel_with_reason(&id, &f.governor, &reason),
        Err(err(RatifyTimelockError::NotGuardian))
    );
    assert_eq!(f.iface.get_operation_state(&id), OperationState::Waiting);
}

#[test]
fn a_cancellation_must_state_a_reason() {
    let f = setup();
    let id = schedule(&f, 750, 1);

    assert_eq!(
        f.timelock
            .try_cancel_with_reason(&id, &f.guardian, &String::from_str(&f.e, "")),
        Err(err(RatifyTimelockError::ReasonRequired))
    );
    assert_eq!(f.iface.get_operation_state(&id), OperationState::Waiting);
}

#[test]
fn the_reasonless_cancel_of_the_standard_interface_is_refused() {
    let f = setup();
    let id = schedule(&f, 750, 1);

    assert_eq!(
        f.iface.try_cancel(&id, &f.guardian),
        Err(err(RatifyTimelockError::ReasonRequired))
    );
    assert_eq!(f.iface.get_operation_state(&id), OperationState::Waiting);
}

#[test]
fn the_guardian_cannot_stop_an_action_that_already_ran() {
    let f = setup();
    let id = schedule(&f, 750, 1);
    f.e.ledger().set_sequence_number(START + DELAY);
    execute(&f, 750, 1);

    let reason = String::from_str(&f.e, "Too late.");
    assert!(f
        .timelock
        .try_cancel_with_reason(&id, &f.guardian, &reason)
        .is_err());
    assert_eq!(f.recorder.recorded(), 750);
}

#[test]
fn the_guardian_has_no_other_power() {
    let f = setup();
    let (target, function, args) = action(&f, 750);

    // Cannot schedule.
    assert_eq!(
        f.iface.try_schedule(
            &target,
            &function,
            &args,
            &zero(&f.e),
            &salt(&f.e, 1),
            &DELAY,
            &f.guardian
        ),
        Err(err(RatifyTimelockError::NotGovernor))
    );

    // Cannot shorten the delay, and neither can anyone else.
    assert_eq!(
        f.iface.try_update_delay(&1u32, &f.guardian),
        Err(err(RatifyTimelockError::ScheduleThroughTimelock))
    );
    assert_eq!(f.iface.get_min_delay(), DELAY);
}

// ################## SELF ADMINISTRATION ##################

#[test]
fn changing_the_delay_must_serve_the_current_delay_first() {
    let f = setup();
    let new_delay = 900u32;
    let s = salt(&f.e, 9);

    let id = f.iface.schedule(
        &f.timelock_id,
        &symbol_short!("set_delay"),
        &Vec::from_array(&f.e, [new_delay.into_val(&f.e)]),
        &zero(&f.e),
        &s,
        &DELAY,
        &f.governor,
    );
    assert_eq!(
        id,
        f.timelock.hash_delay_update(&new_delay, &zero(&f.e), &s)
    );

    // Not yet.
    assert!(f
        .timelock
        .try_execute_delay_update(&new_delay, &zero(&f.e), &s)
        .is_err());
    assert_eq!(f.iface.get_min_delay(), DELAY);

    f.e.ledger().set_sequence_number(START + DELAY);
    f.timelock.execute_delay_update(&new_delay, &zero(&f.e), &s);
    assert_eq!(f.iface.get_min_delay(), new_delay);
    assert_eq!(f.iface.get_operation_state(&id), OperationState::Done);
}

#[test]
fn an_unscheduled_delay_change_does_nothing() {
    let f = setup();
    f.e.ledger().set_sequence_number(START + DELAY);

    assert!(f
        .timelock
        .try_execute_delay_update(&1u32, &zero(&f.e), &salt(&f.e, 9))
        .is_err());
    assert_eq!(f.iface.get_min_delay(), DELAY);
}

#[test]
fn the_delay_can_never_be_set_to_zero() {
    let f = setup();
    assert_eq!(
        f.timelock
            .try_execute_delay_update(&0u32, &zero(&f.e), &salt(&f.e, 9)),
        Err(err(RatifyTimelockError::DelayCannotBeZero))
    );
}

#[test]
fn an_action_already_waiting_keeps_the_delay_it_was_scheduled_under() {
    let f = setup();
    let payment = schedule(&f, 750, 1);

    // Governance lengthens the delay while the payment is waiting.
    let new_delay = 5_000u32;
    let s = salt(&f.e, 9);
    f.iface.schedule(
        &f.timelock_id,
        &symbol_short!("set_delay"),
        &Vec::from_array(&f.e, [new_delay.into_val(&f.e)]),
        &zero(&f.e),
        &s,
        &DELAY,
        &f.governor,
    );
    f.e.ledger().set_sequence_number(START + DELAY);
    f.timelock.execute_delay_update(&new_delay, &zero(&f.e), &s);

    // The payment still becomes ready on its original schedule.
    assert_eq!(f.iface.get_operation_state(&payment), OperationState::Ready);
    execute(&f, 750, 1);
    assert_eq!(f.recorder.recorded(), 750);
}

#[test]
fn governance_replaces_the_guardian_through_the_queue() {
    let f = setup();
    let new_guardian = Address::generate(&f.e);
    let s = salt(&f.e, 4);

    let id = f.iface.schedule(
        &f.timelock_id,
        &symbol_short!("set_guard"),
        &Vec::from_array(&f.e, [new_guardian.clone().into_val(&f.e)]),
        &zero(&f.e),
        &s,
        &DELAY,
        &f.governor,
    );
    assert_eq!(
        id,
        f.timelock
            .hash_guardian_change(&new_guardian, &zero(&f.e), &s)
    );

    f.e.ledger().set_sequence_number(START + DELAY);
    f.timelock
        .execute_guardian_change(&new_guardian, &zero(&f.e), &s);

    assert_eq!(f.timelock.guardian(), new_guardian);

    // The old guardian's brake is gone with the role.
    let waiting = schedule(&f, 750, 1);
    let reason = String::from_str(&f.e, "Still trying.");
    assert_eq!(
        f.timelock
            .try_cancel_with_reason(&waiting, &f.guardian, &reason),
        Err(err(RatifyTimelockError::NotGuardian))
    );
    f.timelock
        .cancel_with_reason(&waiting, &new_guardian, &reason);
    assert_eq!(f.iface.get_operation_state(&waiting), OperationState::Unset);
}

#[test]
fn a_guardian_cannot_hand_the_role_on() {
    let f = setup();
    let successor = Address::generate(&f.e);

    // There is no direct setter at all. The only path is a scheduled
    // operation, which only the governor can create.
    let result = f.iface.try_schedule(
        &f.timelock_id,
        &symbol_short!("set_guard"),
        &Vec::from_array(&f.e, [successor.into_val(&f.e)]),
        &zero(&f.e),
        &salt(&f.e, 4),
        &DELAY,
        &f.guardian,
    );
    assert_eq!(result, Err(err(RatifyTimelockError::NotGovernor)));
    assert_eq!(f.timelock.guardian(), f.guardian);
}
