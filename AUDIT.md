# Audit Status

This document tracks the security audit status of every RatifyDAO contract.

---

## Current status: Not audited

**The contracts have not yet been audited. Do not hold significant funds in a
RatifyDAO treasury on mainnet until an audit has been completed and its
findings are resolved.**

---

## Contracts in scope

| Contract | Version | Lines | Audit |
|---|---|---|---|
| `ratify-treasury` | 0.1.0 | ~300 | Pending |
| `ratify-timelock` | 0.1.0 | ~280 | Pending |
| `ratify-governor` | 0.1.0 | ~350 | Pending |
| `ratify-membership` | 0.1.0 | ~400 | Pending |
| `ratify-weight-rule` | 0.1.0 | ~180 | Pending |
| `ratify-delegate-registry` | 0.1.0 | ~280 | Pending |
| `ratify-factory` | 0.1.0 | ~220 | Pending |

---

## Audit history

No audits have been completed yet.

---

## Self-review notes

Areas that received extra internal review before testnet deployment:

- **Permission chain** — the governor cannot call the treasury directly; every
  payment passes through the timelock and its delay.
- **Spending policy enforcement** — policy limits are checked at execution
  time, not at the vote, so a passed proposal cannot exceed the limits.
- **Snapshot-based voting** — voting power is read at the proposal's snapshot
  ledger, preventing flash-loan attacks.
- **Replay protection** — the timelock refuses to run the same operation twice.
- **Lapsed power** — expired grants stop counting for quorum the moment they
  are swept; nobody can prevent a sweep.

---

## Reporting a vulnerability

See [SECURITY.md](SECURITY.md).
