//! Storage keys, errors and events specific to the Ratify timelock.
//!
//! The scheduling state itself lives in the OpenZeppelin timelock module. What
//! is here is the access control layered on top of it.

use soroban_sdk::{contracterror, contractevent, contracttype, Address, BytesN, String};

#[contracttype]
#[derive(Clone)]
pub enum TimelockStorageKey {
    /// The governor, the only address that may schedule.
    Governor,
    /// The guardian, the only address that may cancel.
    Guardian,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RatifyTimelockError {
    /// The timelock has not been given a governor.
    GovernorNotSet = 1,
    /// The timelock has not been given a guardian.
    GuardianNotSet = 2,
    /// Only the governor may schedule an operation.
    NotGovernor = 3,
    /// Only the guardian may cancel an operation.
    NotGuardian = 4,
    /// A cancellation must state a reason.
    ReasonRequired = 5,
    /// The stated reason is longer than the contract will store.
    ReasonTooLong = 6,
    /// A delay of zero would defeat the purpose of the contract.
    DelayCannotBeZero = 7,
    /// The change must be scheduled and served its delay, not applied
    /// directly.
    ScheduleThroughTimelock = 8,
}

/// Emitted when the guardian pulls the brake on a waiting operation.
///
/// The reason is part of the event, so the queue page can show why an approved
/// decision was stopped.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationCancelledWithReason {
    #[topic]
    pub operation_id: BytesN<32>,
    #[topic]
    pub guardian: Address,
    pub reason: String,
}

/// Emitted when governance replaces the guardian.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuardianChanged {
    #[topic]
    pub old_guardian: Address,
    #[topic]
    pub new_guardian: Address,
}

/// Emitted when governance changes the minimum delay.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DelayUpdated {
    pub old_delay: u32,
    pub new_delay: u32,
}
