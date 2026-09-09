#![no_std]
//! # Ratify timelock
//!
//! The delay between a vote passing and money moving.
//!
//! An approved proposal is scheduled here as an exact call. It waits a fixed
//! number of ledgers that every member can see and object during. When the
//! delay ends, **anyone** may trigger it, so finishing the job does not depend
//! on an operator being willing.
//!
//! Three roles, and no others:
//!
//! * the **governor** may schedule, and may do nothing else
//! * **anyone** may execute a ready operation
//! * the **guardian** may cancel a waiting operation, with a stated reason,
//!   and may do nothing else
//!
//! Changing the delay, or replacing the guardian, is done by scheduling the
//! change here like any other action, so it waits out the current delay before
//! it takes effect. There is no admin key.
//!
//! Scheduling, replay protection and the waiting/ready/done state machine come
//! from the OpenZeppelin Stellar governance library. What is written here is
//! the access control: who may do each of the three things, the requirement
//! that a cancellation carries a reason, and the self-administration path.

mod types;

#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contractimpl, panic_with_error, symbol_short, Address, BytesN, Env, IntoVal, String,
    Symbol, Val, Vec,
};
use stellar_governance::timelock::{
    cancel_operation, execute_operation, hash_operation, schedule_operation, set_execute_operation,
    set_min_delay, Operation, OperationState, Timelock,
};

pub use crate::types::{
    RatifyTimelockError, DelayUpdated, GuardianChanged, OperationCancelledWithReason,
    TimelockStorageKey,
};

/// The longest a cancellation reason may be, in bytes.
const MAX_REASON_LENGTH: u32 = 512;

/// The function name a scheduled delay change is recorded under.
const FN_SET_DELAY: Symbol = symbol_short!("set_delay");

/// The function name a scheduled guardian change is recorded under.
const FN_SET_GUARD: Symbol = symbol_short!("set_guard");

#[contract]
pub struct RatifyTimelock;

#[contractimpl]
impl RatifyTimelock {
    /// Binds the timelock to its governor and guardian and sets the delay.
    ///
    /// The governor cannot be changed. A community that wants a different
    /// governor deploys a new set of contracts, which is visible to every
    /// member, rather than having the change happen quietly.
    pub fn __constructor(e: &Env, governor: Address, guardian: Address, min_delay: u32) {
        if min_delay == 0 {
            panic_with_error!(e, RatifyTimelockError::DelayCannotBeZero);
        }
        e.storage().instance().set(&TimelockStorageKey::Governor, &governor);
        e.storage().instance().set(&TimelockStorageKey::Guardian, &guardian);
        set_min_delay(e, min_delay);
    }

    // ################## QUERIES ##################

    /// Returns the governor, the only address that may schedule an operation.
    pub fn governor(e: &Env) -> Address {
        e.storage()
            .instance()
            .get(&TimelockStorageKey::Governor)
            .unwrap_or_else(|| panic_with_error!(e, RatifyTimelockError::GovernorNotSet))
    }

    /// Returns the guardian, the only address that may cancel one.
    pub fn guardian(e: &Env) -> Address {
        e.storage()
            .instance()
            .get(&TimelockStorageKey::Guardian)
            .unwrap_or_else(|| panic_with_error!(e, RatifyTimelockError::GuardianNotSet))
    }

    /// Returns how many ledgers remain before an operation may be executed.
    ///
    /// Zero means it is ready now, or was never scheduled. The queue page
    /// reads this for its countdown alongside
    /// [`get_operation_state`](Timelock::get_operation_state), so it can tell
    /// those two apart.
    pub fn ledgers_remaining(e: &Env, operation_id: BytesN<32>) -> u32 {
        match Self::get_operation_state(e, operation_id.clone()) {
            OperationState::Waiting => {
                let ready_at = Self::get_operation_ledger(e, operation_id);
                ready_at.saturating_sub(e.ledger().sequence())
            }
            _ => 0,
        }
    }

    // ################## THE BRAKE ##################

    /// Cancels a waiting operation, with a reason recorded on chain.
    ///
    /// This is the emergency brake, and it is the guardian's only power. The
    /// guardian cannot execute, cannot move funds, cannot alter policy and
    /// cannot create proposals.
    ///
    /// The reason is required. A brake that can be pulled silently is a brake
    /// nobody can be held to account for.
    pub fn cancel_with_reason(
        e: &Env,
        operation_id: BytesN<32>,
        guardian: Address,
        reason: String,
    ) {
        let expected = Self::guardian(e);
        if guardian != expected {
            panic_with_error!(e, RatifyTimelockError::NotGuardian);
        }
        guardian.require_auth();
        if reason.len() == 0 {
            panic_with_error!(e, RatifyTimelockError::ReasonRequired);
        }
        if reason.len() > MAX_REASON_LENGTH {
            panic_with_error!(e, RatifyTimelockError::ReasonTooLong);
        }
        cancel_operation(e, &operation_id);
        OperationCancelledWithReason { operation_id, guardian, reason }.publish(e);
    }

    // ################## SELF ADMINISTRATION ##################
    //
    // Soroban does not allow a contract to call itself, so a change to the
    // timelock's own settings cannot arrive through `execute` like a payment
    // does. Instead each self-administered change has its own entry point.
    // Both go through the same schedule, the same delay and the same replay
    // protection as any other operation; only the dispatch differs.

    /// Returns the identifier a scheduled delay change will have.
    ///
    /// The governor schedules the change against this contract using
    /// [`Timelock::schedule`] with the same arguments. Publishing the hash
    /// here means a member can check that what was queued is what the
    /// proposal said.
    pub fn hash_delay_update(
        e: &Env,
        new_delay: u32,
        predecessor: BytesN<32>,
        salt: BytesN<32>,
    ) -> BytesN<32> {
        hash_operation(e, &Self::delay_operation(e, new_delay, predecessor, salt))
    }

    /// Returns the identifier a scheduled guardian change will have.
    pub fn hash_guardian_change(
        e: &Env,
        new_guardian: Address,
        predecessor: BytesN<32>,
        salt: BytesN<32>,
    ) -> BytesN<32> {
        hash_operation(e, &Self::guardian_operation(e, new_guardian, predecessor, salt))
    }

    /// Applies a scheduled change to the minimum delay.
    ///
    /// Open to anyone once the delay has passed, like any other execution.
    /// Operations already waiting keep the delay they were scheduled under, so
    /// shortening the delay never shortens a review already in progress.
    pub fn execute_delay_update(
        e: &Env,
        new_delay: u32,
        predecessor: BytesN<32>,
        salt: BytesN<32>,
    ) {
        if new_delay == 0 {
            panic_with_error!(e, RatifyTimelockError::DelayCannotBeZero);
        }
        let operation = Self::delay_operation(e, new_delay, predecessor, salt);
        set_execute_operation(e, &operation);

        let old_delay = Self::get_min_delay(e);
        set_min_delay(e, new_delay);
        DelayUpdated { old_delay, new_delay }.publish(e);
    }

    /// Applies a scheduled change of guardian.
    ///
    /// Governance grants and revokes the role. The guardian cannot pass it on.
    pub fn execute_guardian_change(
        e: &Env,
        new_guardian: Address,
        predecessor: BytesN<32>,
        salt: BytesN<32>,
    ) {
        let operation = Self::guardian_operation(e, new_guardian.clone(), predecessor, salt);
        set_execute_operation(e, &operation);

        let old_guardian = Self::guardian(e);
        e.storage().instance().set(&TimelockStorageKey::Guardian, &new_guardian);
        GuardianChanged { old_guardian, new_guardian }.publish(e);
    }

    // ################## INTERNAL ##################

    fn delay_operation(
        e: &Env,
        new_delay: u32,
        predecessor: BytesN<32>,
        salt: BytesN<32>,
    ) -> Operation {
        Operation {
            target: e.current_contract_address(),
            function: FN_SET_DELAY,
            args: Vec::from_array(e, [new_delay.into_val(e)]),
            predecessor,
            salt,
        }
    }

    fn guardian_operation(
        e: &Env,
        new_guardian: Address,
        predecessor: BytesN<32>,
        salt: BytesN<32>,
    ) -> Operation {
        Operation {
            target: e.current_contract_address(),
            function: FN_SET_GUARD,
            args: Vec::from_array(e, [new_guardian.into_val(e)]),
            predecessor,
            salt,
        }
    }
}

#[contractimpl(contracttrait)]
impl Timelock for RatifyTimelock {
    /// Schedules an approved action.
    ///
    /// Only the governor may schedule, and only for at least the minimum
    /// delay. A shorter delay is refused rather than rounded up, so a proposal
    /// cannot quietly shorten its own review period.
    fn schedule(
        e: &Env,
        target: Address,
        function: Symbol,
        args: Vec<Val>,
        predecessor: BytesN<32>,
        salt: BytesN<32>,
        delay: u32,
        proposer: Address,
    ) -> BytesN<32> {
        let governor = Self::governor(e);
        if proposer != governor {
            panic_with_error!(e, RatifyTimelockError::NotGovernor);
        }
        proposer.require_auth();
        let operation = Operation { target, function, args, predecessor, salt };
        schedule_operation(e, &operation, delay)
    }

    /// Executes an operation whose delay has passed.
    ///
    /// Execution is open. Anyone may call this and the `executor` argument is
    /// ignored, because a decision that has been approved and has waited out
    /// its delay should not need anyone's permission to take effect. The
    /// library refuses an operation that is not ready and refuses to run one
    /// twice.
    fn execute(
        e: &Env,
        target: Address,
        function: Symbol,
        args: Vec<Val>,
        predecessor: BytesN<32>,
        salt: BytesN<32>,
        _executor: Option<Address>,
    ) -> Val {
        let operation = Operation { target, function, args, predecessor, salt };
        execute_operation(e, &operation)
    }

    /// Cancels a waiting operation. Always fails.
    ///
    /// RatifyDAO requires a stated reason for every cancellation, so
    /// [`RatifyTimelock::cancel_with_reason`] is the only way to pull the
    /// brake. This method exists because the standard timelock interface
    /// declares it, and refusing here is what keeps the reason from being
    /// optional.
    fn cancel(e: &Env, _operation_id: BytesN<32>, _canceller: Address) {
        panic_with_error!(e, RatifyTimelockError::ReasonRequired);
    }

    /// Changes the minimum delay. Always fails.
    ///
    /// A delay change has to serve the current delay before it applies, so it
    /// is scheduled like any other action and applied through
    /// [`RatifyTimelock::execute_delay_update`]. Allowing an immediate change
    /// here would let a passing proposal remove the review period it was
    /// meant to be subject to.
    fn update_delay(e: &Env, _new_delay: u32, _operator: Address) {
        panic_with_error!(e, RatifyTimelockError::ScheduleThroughTimelock);
    }
}
