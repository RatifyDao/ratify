#![cfg(test)]
extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token, Address, BytesN, Env, Error, InvokeError,
};

use crate::{RatifyTreasury, RatifyTreasuryClient, TreasuryError};

/// The shape a `try_` call returns when the contract panicked with one of our
/// errors, so a test can name the rule that refused it.
fn err(code: TreasuryError) -> Result<Error, InvokeError> {
    Ok(Error::from_contract_error(code as u32))
}

const WINDOW: u32 = 1_000;

struct Fixture<'a> {
    e: Env,
    timelock: Address,
    treasury: RatifyTreasuryClient<'a>,
    asset: Address,
    mint: token::StellarAssetClient<'a>,
    token: token::TokenClient<'a>,
}

/// A treasury bound to a timelock, funded with a million units of one asset.
///
/// Authorisation is mocked, so `require_auth` on the timelock passes. The
/// tests that care about who may call assert against the unmocked env
/// separately.
fn setup<'a>() -> Fixture<'a> {
    let e = Env::default();
    e.mock_all_auths();
    e.ledger().set_sequence_number(10_000);

    let timelock = Address::generate(&e);
    let treasury_id = e.register(RatifyTreasury, (timelock.clone(),));
    let treasury = RatifyTreasuryClient::new(&e, &treasury_id);

    let issuer = Address::generate(&e);
    let asset_contract = e.register_stellar_asset_contract_v2(issuer);
    let asset = asset_contract.address();
    let mint = token::StellarAssetClient::new(&e, &asset);
    let token = token::TokenClient::new(&e, &asset);
    mint.mint(&treasury_id, &1_000_000);

    Fixture {
        e,
        timelock,
        treasury,
        asset,
        mint,
        token,
    }
}

fn proposal_id(e: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(e, &[byte; 32])
}

/// Admits an asset with generous limits, so a test can focus on one rule.
fn permit(f: &Fixture, per_payment: i128, window: i128) {
    f.treasury
        .set_policy(&f.asset, &per_payment, &window, &WINDOW);
}

// ################## THE PERMISSION CHAIN ##################

#[test]
fn constructor_records_the_timelock() {
    let f = setup();
    assert_eq!(f.treasury.timelock(), f.timelock);
}

#[test]
fn payment_requires_the_timelock() {
    let f = setup();
    permit(&f, 1_000, 10_000);

    // Without mocked auth there is no signature from the timelock, so the
    // payment cannot go through however well formed it is.
    let stranger = Address::generate(&f.e);
    f.e.set_auths(&[]);
    let result = f
        .treasury
        .try_pay(&f.asset, &stranger, &100, &proposal_id(&f.e, 1));
    assert!(result.is_err());
}

#[test]
fn policy_changes_require_the_timelock() {
    let f = setup();
    f.e.set_auths(&[]);
    let result = f
        .treasury
        .try_set_policy(&f.asset, &1_000, &10_000, &WINDOW);
    assert!(result.is_err());
}

// ################## THE ASSET ALLOWLIST ##################

#[test]
fn an_asset_without_a_policy_cannot_leave() {
    let f = setup();
    let to = Address::generate(&f.e);

    let result = f.treasury.try_pay(&f.asset, &to, &1, &proposal_id(&f.e, 1));
    assert_eq!(result, Err(err(TreasuryError::AssetNotAllowed)));
    assert_eq!(f.token.balance(&to), 0);
}

#[test]
fn withdrawing_a_policy_bars_the_asset_again() {
    let f = setup();
    let to = Address::generate(&f.e);
    permit(&f, 1_000, 10_000);
    f.treasury.pay(&f.asset, &to, &100, &proposal_id(&f.e, 1));

    f.treasury.remove_policy(&f.asset);

    let result = f
        .treasury
        .try_pay(&f.asset, &to, &100, &proposal_id(&f.e, 2));
    assert_eq!(result, Err(err(TreasuryError::AssetNotAllowed)));
    assert_eq!(f.token.balance(&to), 100);
}

#[test]
fn a_policy_must_have_usable_limits() {
    let f = setup();
    assert_eq!(
        f.treasury.try_set_policy(&f.asset, &0, &10_000, &WINDOW),
        Err(err(TreasuryError::InvalidPolicy))
    );
    assert_eq!(
        f.treasury.try_set_policy(&f.asset, &1_000, &0, &WINDOW),
        Err(err(TreasuryError::InvalidPolicy))
    );
    assert_eq!(
        f.treasury.try_set_policy(&f.asset, &1_000, &10_000, &0),
        Err(err(TreasuryError::InvalidPolicy))
    );
}

// ################## THE AMOUNT LIMITS ##################

#[test]
fn a_payment_within_every_limit_goes_through() {
    let f = setup();
    let to = Address::generate(&f.e);
    permit(&f, 1_000, 10_000);

    let index = f.treasury.pay(&f.asset, &to, &750, &proposal_id(&f.e, 7));

    assert_eq!(index, 0);
    assert_eq!(f.token.balance(&to), 750);
    assert_eq!(f.treasury.balance(&f.asset), 999_250);
}

#[test]
fn a_payment_over_the_per_payment_cap_is_refused() {
    let f = setup();
    let to = Address::generate(&f.e);
    permit(&f, 1_000, 100_000);

    let result = f
        .treasury
        .try_pay(&f.asset, &to, &1_001, &proposal_id(&f.e, 1));
    assert_eq!(result, Err(err(TreasuryError::OverPerPaymentCap)));
    assert_eq!(f.token.balance(&to), 0);
}

#[test]
fn payments_at_the_cap_exactly_are_allowed() {
    let f = setup();
    let to = Address::generate(&f.e);
    permit(&f, 1_000, 1_000);

    f.treasury.pay(&f.asset, &to, &1_000, &proposal_id(&f.e, 1));
    assert_eq!(f.token.balance(&to), 1_000);
}

#[test]
fn a_run_of_payments_over_the_window_cap_is_refused() {
    let f = setup();
    let to = Address::generate(&f.e);
    permit(&f, 1_000, 2_500);

    f.treasury.pay(&f.asset, &to, &1_000, &proposal_id(&f.e, 1));
    f.treasury.pay(&f.asset, &to, &1_000, &proposal_id(&f.e, 2));
    assert_eq!(f.treasury.window_spent(&f.asset), 2_000);
    assert_eq!(f.treasury.window_headroom(&f.asset), 500);

    // Each payment is inside the per-payment cap. The third breaches the
    // rolling total, which is the limit the vote cannot argue with.
    let result = f
        .treasury
        .try_pay(&f.asset, &to, &1_000, &proposal_id(&f.e, 3));
    assert_eq!(result, Err(err(TreasuryError::OverWindowCap)));
    assert_eq!(f.token.balance(&to), 2_000);
}

#[test]
fn the_window_frees_up_as_it_rolls_forward() {
    let f = setup();
    let to = Address::generate(&f.e);
    permit(&f, 1_000, 1_000);

    f.treasury.pay(&f.asset, &to, &1_000, &proposal_id(&f.e, 1));
    assert_eq!(f.treasury.window_headroom(&f.asset), 0);

    // A payment counts for exactly `window_ledgers` ledgers, so it is still
    // inside the window on the last of them.
    f.e.ledger().set_sequence_number(10_000 + WINDOW - 1);
    assert_eq!(f.treasury.window_spent(&f.asset), 1_000);
    assert_eq!(
        f.treasury.try_pay(&f.asset, &to, &1, &proposal_id(&f.e, 2)),
        Err(err(TreasuryError::OverWindowCap))
    );

    // A ledger later it has aged out and the cap is free again.
    f.e.ledger().set_sequence_number(10_000 + WINDOW);
    assert_eq!(f.treasury.window_spent(&f.asset), 0);
    f.treasury.pay(&f.asset, &to, &1_000, &proposal_id(&f.e, 3));
    assert_eq!(f.token.balance(&to), 2_000);
}

#[test]
fn a_zero_or_negative_payment_is_refused() {
    let f = setup();
    let to = Address::generate(&f.e);
    permit(&f, 1_000, 10_000);

    assert_eq!(
        f.treasury.try_pay(&f.asset, &to, &0, &proposal_id(&f.e, 1)),
        Err(err(TreasuryError::InvalidAmount))
    );
    assert_eq!(
        f.treasury
            .try_pay(&f.asset, &to, &-100, &proposal_id(&f.e, 2)),
        Err(err(TreasuryError::InvalidAmount))
    );
}

// ################## THE DESTINATION ALLOWLIST ##################

#[test]
fn destinations_are_open_until_governance_restricts_them() {
    let f = setup();
    let to = Address::generate(&f.e);
    permit(&f, 1_000, 10_000);

    assert!(!f.treasury.destinations_restricted());
    assert!(f.treasury.destination_allowed(&to));
    f.treasury.pay(&f.asset, &to, &100, &proposal_id(&f.e, 1));
    assert_eq!(f.token.balance(&to), 100);
}

#[test]
fn a_restricted_treasury_pays_allowed_destinations_only() {
    let f = setup();
    let allowed = Address::generate(&f.e);
    let stranger = Address::generate(&f.e);
    permit(&f, 1_000, 10_000);

    f.treasury.set_destination_restriction(&true);
    f.treasury.set_destination(&allowed, &true);

    f.treasury
        .pay(&f.asset, &allowed, &100, &proposal_id(&f.e, 1));
    assert_eq!(f.token.balance(&allowed), 100);

    let result = f
        .treasury
        .try_pay(&f.asset, &stranger, &100, &proposal_id(&f.e, 2));
    assert_eq!(result, Err(err(TreasuryError::DestinationNotAllowed)));
    assert_eq!(f.token.balance(&stranger), 0);
}

#[test]
fn a_destination_can_be_taken_off_the_allowlist() {
    let f = setup();
    let to = Address::generate(&f.e);
    permit(&f, 1_000, 10_000);
    f.treasury.set_destination_restriction(&true);
    f.treasury.set_destination(&to, &true);
    f.treasury.pay(&f.asset, &to, &100, &proposal_id(&f.e, 1));

    f.treasury.set_destination(&to, &false);

    assert!(!f.treasury.destination_allowed(&to));
    assert_eq!(
        f.treasury
            .try_pay(&f.asset, &to, &100, &proposal_id(&f.e, 2)),
        Err(err(TreasuryError::DestinationNotAllowed))
    );
}

// ################## THE PUBLIC RECORD ##################

#[test]
fn every_payment_is_recorded_against_its_proposal() {
    let f = setup();
    let first = Address::generate(&f.e);
    let second = Address::generate(&f.e);
    permit(&f, 1_000, 10_000);

    f.treasury
        .pay(&f.asset, &first, &100, &proposal_id(&f.e, 1));
    f.e.ledger().set_sequence_number(10_050);
    f.treasury
        .pay(&f.asset, &second, &250, &proposal_id(&f.e, 2));

    assert_eq!(f.treasury.outflow_count(), 2);

    let one = f.treasury.outflow(&0);
    assert_eq!(one.to, first);
    assert_eq!(one.amount, 100);
    assert_eq!(one.ledger, 10_000);
    assert_eq!(one.proposal_id, proposal_id(&f.e, 1));

    let two = f.treasury.outflow(&1);
    assert_eq!(two.to, second);
    assert_eq!(two.amount, 250);
    assert_eq!(two.ledger, 10_050);
    assert_eq!(two.proposal_id, proposal_id(&f.e, 2));
}

#[test]
fn a_refused_payment_leaves_no_record() {
    let f = setup();
    let to = Address::generate(&f.e);
    permit(&f, 100, 10_000);

    let _ = f
        .treasury
        .try_pay(&f.asset, &to, &500, &proposal_id(&f.e, 1));

    assert_eq!(f.treasury.outflow_count(), 0);
    assert_eq!(f.treasury.window_spent(&f.asset), 0);
    assert_eq!(f.treasury.balance(&f.asset), 1_000_000);
}

// ################## THE PREVIEW ##################

#[test]
fn would_allow_agrees_with_what_pay_does() {
    let f = setup();
    let to = Address::generate(&f.e);
    permit(&f, 1_000, 1_500);

    assert!(f.treasury.would_allow(&f.asset, &to, &1_000));
    assert!(!f.treasury.would_allow(&f.asset, &to, &1_001));
    assert!(!f.treasury.would_allow(&f.asset, &to, &0));

    f.treasury.pay(&f.asset, &to, &1_000, &proposal_id(&f.e, 1));

    // The window now has 500 left, so the same payment that was fine a moment
    // ago is not.
    assert!(!f.treasury.would_allow(&f.asset, &to, &1_000));
    assert!(f.treasury.would_allow(&f.asset, &to, &500));
}

#[test]
fn would_allow_refuses_an_unlisted_asset_and_destination() {
    let f = setup();
    let to = Address::generate(&f.e);

    assert!(!f.treasury.would_allow(&f.asset, &to, &100));

    permit(&f, 1_000, 10_000);
    f.treasury.set_destination_restriction(&true);
    assert!(!f.treasury.would_allow(&f.asset, &to, &100));

    f.treasury.set_destination(&to, &true);
    assert!(f.treasury.would_allow(&f.asset, &to, &100));
}

// ################## FUNDING ##################

#[test]
fn anyone_can_fund_a_treasury() {
    let f = setup();
    let donor = Address::generate(&f.e);
    f.mint.mint(&donor, &5_000);

    f.treasury.deposit(&donor, &f.asset, &5_000);

    assert_eq!(f.treasury.balance(&f.asset), 1_005_000);
    assert_eq!(f.token.balance(&donor), 0);
}

#[test]
fn a_deposit_of_nothing_is_refused() {
    let f = setup();
    let donor = Address::generate(&f.e);
    assert_eq!(
        f.treasury.try_deposit(&donor, &f.asset, &0),
        Err(err(TreasuryError::InvalidAmount))
    );
}

// ################## LIMITS OF THE WINDOW LEDGER ##################

#[test]
fn the_window_ledger_is_bounded() {
    let f = setup();
    let to = Address::generate(&f.e);
    permit(&f, 10, 100_000);

    for i in 0..100u32 {
        f.e.ledger().set_sequence_number(10_000 + i);
        f.treasury.pay(&f.asset, &to, &1, &proposal_id(&f.e, 1));
    }

    // The hundred and first payment inside one window is refused rather than
    // silently dropping the oldest, which would understate the total spent.
    assert_eq!(
        f.treasury.try_pay(&f.asset, &to, &1, &proposal_id(&f.e, 2)),
        Err(err(TreasuryError::WindowFull))
    );

    // Rolling past the window clears it.
    f.e.ledger().set_sequence_number(10_000 + WINDOW + 100);
    f.treasury.pay(&f.asset, &to, &1, &proposal_id(&f.e, 3));
    assert_eq!(f.token.balance(&to), 101);
}
