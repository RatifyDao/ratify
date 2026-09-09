//! Storage keys, the spending policy, and the events the treasury emits.

use soroban_sdk::{contracterror, contractevent, contracttype, Address, BytesN};

/// The spending policy for a single asset.
///
/// A policy exists per asset. An asset with no policy cannot leave the
/// treasury at all, so the allowlist of assets is simply the set of assets
/// that have one. Every field is a ceiling the contract enforces at payment
/// time, regardless of what the vote said.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetPolicy {
    /// The largest amount a single proposal may move.
    pub per_payment_cap: i128,
    /// The largest total that may leave within `window_ledgers`.
    pub window_cap: i128,
    /// The length of the rolling window, in ledgers. A payment counts
    /// against the window cap for exactly this many ledgers, then ages out.
    pub window_ledgers: u32,
}

/// A payment that has left the treasury.
///
/// Retained so the rolling window can be measured, and so the treasury page
/// can show every outflow ever made against the proposal that authorised it.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Outflow {
    pub asset: Address,
    pub to: Address,
    pub amount: i128,
    pub ledger: u32,
    /// The governor proposal that authorised this payment.
    pub proposal_id: BytesN<32>,
}

#[contracttype]
#[derive(Clone)]
pub enum TreasuryStorageKey {
    /// The timelock. The only address whose instructions this contract obeys.
    Timelock,
    /// Whether payments are restricted to the destination allowlist.
    RestrictDestinations,
    /// Per-asset spending policy.
    Policy(Address),
    /// Payments made against an asset inside the current rolling window.
    Window(Address),
    /// Membership of the destination allowlist.
    Destination(Address),
    /// Total number of outflows recorded.
    OutflowCount,
    /// An individual outflow, by index.
    Outflow(u32),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TreasuryError {
    /// The treasury has not been given a timelock address.
    TimelockNotSet = 1,
    /// The asset has no policy, so it cannot leave the treasury.
    AssetNotAllowed = 2,
    /// The payment exceeds the per-payment cap for this asset.
    OverPerPaymentCap = 3,
    /// The payment would exceed the rolling window cap for this asset.
    OverWindowCap = 4,
    /// Destinations are restricted and this one is not on the allowlist.
    DestinationNotAllowed = 5,
    /// The amount is zero or negative.
    InvalidAmount = 6,
    /// A policy field is not usable (a non-positive cap, or a zero window).
    InvalidPolicy = 7,
    /// Too many payments against one asset inside the window to track.
    WindowFull = 8,
    /// The requested outflow index does not exist.
    OutflowNotFound = 9,
}

/// Emitted when funds leave the treasury.
///
/// This is the event the whole product exists to produce.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentMade {
    #[topic]
    pub asset: Address,
    #[topic]
    pub to: Address,
    #[topic]
    pub proposal_id: BytesN<32>,
    pub amount: i128,
    pub index: u32,
}

/// Emitted when funds are deposited through [`deposit`](crate::RatifyTreasury::deposit).
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepositMade {
    #[topic]
    pub asset: Address,
    #[topic]
    pub from: Address,
    pub amount: i128,
}

/// Emitted when governance changes the policy for an asset.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicySet {
    #[topic]
    pub asset: Address,
    pub per_payment_cap: i128,
    pub window_cap: i128,
    pub window_ledgers: u32,
}

/// Emitted when governance withdraws an asset's policy, barring it from
/// leaving the treasury.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyRemoved {
    #[topic]
    pub asset: Address,
}

/// Emitted when a destination is added to or removed from the allowlist.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DestinationSet {
    #[topic]
    pub destination: Address,
    pub allowed: bool,
}

/// Emitted when governance turns destination restriction on or off.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DestinationRestrictionSet {
    pub restricted: bool,
}
