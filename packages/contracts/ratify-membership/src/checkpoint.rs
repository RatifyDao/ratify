//! Voting power recorded against the ledger it changed on.
//!
//! A proposal is decided on the power that existed at its snapshot ledger, not
//! the power that exists when the vote is counted. Otherwise a member could
//! acquire power after seeing how a vote is going.
//!
//! Each series is stored as a count plus individually keyed entries, so a read
//! at a past ledger costs a binary search rather than loading the whole
//! history.

use soroban_sdk::{contracttype, Address, Env};

/// A recorded value and the ledger from which it applied.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Checkpoint {
    pub ledger: u32,
    pub value: u128,
}

/// The series a checkpoint belongs to.
///
/// Delegated power is per account. Live total is the one series that decides
/// whether a quorum has been reached.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Series {
    /// Voting power currently answering to this account.
    Power(Address),
    /// Voting power across the community that has not lapsed.
    LiveTotal,
    /// The number of members whose live grant answers to this account.
    /// One member one vote is counted from here, not from tokens held.
    Heads(Address),
    /// The number of members across the community with a live grant.
    LiveHeads,
}

#[contracttype]
#[derive(Clone)]
pub enum CheckpointKey {
    Count(Series),
    At(Series, u32),
}

const EXTEND_AMOUNT: u32 = 30 * 17_280;
const TTL_THRESHOLD: u32 = EXTEND_AMOUNT - 17_280;

/// Returns how many entries a series holds.
pub fn count(e: &Env, series: &Series) -> u32 {
    e.storage()
        .persistent()
        .get(&CheckpointKey::Count(series.clone()))
        .unwrap_or(0)
}

/// Returns the value of a series as it stands now.
pub fn latest(e: &Env, series: &Series) -> u128 {
    let count = count(e, series);
    if count == 0 {
        return 0;
    }
    entry(e, series, count - 1).value
}

/// Returns the value a series held at `ledger`.
///
/// A ledger before the first entry reads as zero, which is correct: there was
/// no power then.
pub fn value_at(e: &Env, series: &Series, ledger: u32) -> u128 {
    let count = count(e, series);
    if count == 0 {
        return 0;
    }

    // The common case, asked for by every vote on a live proposal.
    let last = entry(e, series, count - 1);
    if last.ledger <= ledger {
        return last.value;
    }

    let first = entry(e, series, 0);
    if first.ledger > ledger {
        return 0;
    }

    // Binary search for the last entry at or before `ledger`.
    let mut low = 0u32;
    let mut high = count - 1;
    while low < high {
        let mid = high - (high - low) / 2;
        if entry(e, series, mid).ledger <= ledger {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    entry(e, series, low).value
}

/// Records a new value for a series from the current ledger onwards.
///
/// Several changes inside one ledger collapse into a single entry, so the
/// series never holds two values for the same ledger.
pub fn record(e: &Env, series: &Series, value: u128) {
    let now = e.ledger().sequence();
    let count = count(e, series);

    if count > 0 {
        let last = entry(e, series, count - 1);
        if last.ledger == now {
            write(e, series, count - 1, &Checkpoint { ledger: now, value });
            return;
        }
    }

    write(e, series, count, &Checkpoint { ledger: now, value });
    e.storage()
        .persistent()
        .set(&CheckpointKey::Count(series.clone()), &(count + 1));
    extend(e, &CheckpointKey::Count(series.clone()));
}

/// Adds to a series, saturating at zero on the way down.
pub fn adjust(e: &Env, series: &Series, delta: i128) {
    let current = latest(e, series);
    let next = if delta >= 0 {
        current + delta as u128
    } else {
        current.saturating_sub(delta.unsigned_abs())
    };
    record(e, series, next);
}

fn entry(e: &Env, series: &Series, index: u32) -> Checkpoint {
    e.storage()
        .persistent()
        .get(&CheckpointKey::At(series.clone(), index))
        .unwrap_or(Checkpoint {
            ledger: 0,
            value: 0,
        })
}

fn write(e: &Env, series: &Series, index: u32, checkpoint: &Checkpoint) {
    let key = CheckpointKey::At(series.clone(), index);
    e.storage().persistent().set(&key, checkpoint);
    extend(e, &key);
}

fn extend(e: &Env, key: &CheckpointKey) {
    e.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, EXTEND_AMOUNT);
}
