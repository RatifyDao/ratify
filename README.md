<p align="center">
  <img src="assets/logo.svg" alt="RatifyDAO" width="420"/>
</p>

<p align="center">
  <strong>Modular on-chain governance on Stellar — where the vote actually moves the money.</strong>
</p>

<p align="center">
  <a href="LICENSE">
    <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License"/>
  </a>
  <a href="AUDIT.md">
    <img src="https://img.shields.io/badge/audit-pending-orange.svg" alt="Audit pending"/>
  </a>
  <img src="https://img.shields.io/badge/network-Stellar%20Testnet-6366f1.svg" alt="Stellar Testnet"/>
</p>

---

A DAO platform built on Soroban with a real treasury, an enforced delay before
action, and delegates who carry a public record.

The short version. A proposal that passes is queued, not executed. It waits a
fixed delay that every member can see and object during. When the delay ends,
anyone can trigger it, and the treasury pays out inside limits the contract
enforces rather than the operator. Delegated power carries a term and lapses
unless it is renewed, so a quorum reflects members who are actually present.

## What is here

| Part | State |
|---|---|
| Seven Soroban contracts | Built, 145 tests |
| The indexer | Built, following testnet |
| Ten screens | Built, reading the index |
| A live deployment | On testnet, with a payment made by vote |

`ratify.testnet.json` holds the deployed addresses. The community there has
issued membership, granted voting power, set a spending policy by vote and paid
4 XLM to a recipient by vote, each through the full path with a real delay in
between.

## Contracts

Seven, each with one responsibility.

| Contract | Responsibility |
|---|---|
| `ratify-treasury` | Holds funds. Pays out only on instruction from the timelock, inside policy. |
| `ratify-timelock` | Holds an approved action for the delay, then lets anyone execute it. |
| `ratify-membership` | Non-transferable membership tokens, and voting power that lapses. |
| `ratify-weight-rule` | Answers how much an account's vote is worth at a point in time. |
| `ratify-governor` | Proposals, votes, and the decision to queue. Queuing is on. |
| `ratify-delegate-registry` | Participation and voting record for every delegate. |
| `ratify-factory` | Deploys and wires a full community set in one transaction. |

### The permission chain

This is the part that makes the product safe.

- The governor can queue an action into the timelock. It cannot execute and it
  cannot touch the treasury.
- The timelock can call the treasury. It cannot do so before the delay has
  passed, and it cannot run the same action twice.
- The treasury accepts instructions from the timelock only, and rejects any
  that breach policy, whatever the vote said.
- The guardian can cancel a queued item, with a stated reason. It has no other
  power, and governance grants and revokes the role.

No deployer key, operator key or frontend can move funds at any point in that
chain. The factory sets the whole chain up in one transaction that either
succeeds or reverts, so there is no half-built community.

## Running it

```bash
bash scripts/test.sh                     # every contract test, including a real deployment
stellar contract build                   # wasm for the ledger

cd indexer && npm install && npm start    # follow the chain into the index
cd web     && npm install && npm run dev  # the interface, at localhost:3000
```

Copy `.env.example` to `.env` and set `RATIFY_FACTORY_ID` first. Both the
indexer and the interface read it.

`scripts/test.sh` builds the wasm before running the suite, because the
factory's tests deploy a real community from real compiled contracts. Those
tests sit behind the `wasm-tests` feature so a plain `cargo test` still works
from a fresh clone.

`stellar contract build` rather than `cargo build --target wasm32v1-none`: the
OpenZeppelin crates enable the SDK's spec shaking feature, which needs the CLI
to finish the job. stellar-cli 25.2.0 or newer.

### Putting a proposal through on testnet

```bash
cd indexer
node scripts/seed-testnet.mjs policy    # set a spending policy, by vote
node scripts/seed-testnet.mjs pay       # pay someone, by vote
node scripts/seed-testnet.mjs settle    # settle the delegate records
```

Each runs the whole path: propose, vote, queue, wait out the delay, execute. It
takes a few minutes of real time because the delay is real. Every stage is a
separate transaction that anyone could send, and after the delay the execution
could be sent by a stranger.

The Stellar CLI cannot express a proposal's arguments, which are a list of
lists of contract values it has no way to type. That is why this is a script
rather than a few CLI calls.

### On Windows

Run this once after cloning:

```bash
bash scripts/windows-dev-setup.sh
```

Soroban contract crates declare a `cdylib` target, which is how
`stellar contract build` finds them. Linking one of those natively on a
windows-gnu toolchain re-exports the whole Soroban host through a single DLL
and lld gives up with `export ordinal too large`. It only bites `cargo test`,
only on Windows, and only for a contract another contract depends on.

The script writes a gitignored `.cargo/config.toml` that links this workspace's
native DLLs with no exports at all, and patches the OpenZeppelin crates to drop
their cdylib. Nothing tracked changes, the project still depends on the
published crates at the pinned versions, and the wasm build never needed any of
it.

## The index

Events go into SQLite as they happen and the interface reads that. The
chain stays the source of truth and the index is a read model: every handler is
safe to run twice, so it can be emptied and replayed, and losing it costs the
time to rebuild and nothing else. `npm run reset` in `indexer/` does exactly
that.

One thing the index derives rather than trusts. The treasury takes a proposal
identifier as an argument to a payment and has no way to check it, so a
proposal could name any identifier it liked. The real link is the transaction:
funds can only leave through the timelock, and the governor marks the proposal
executed in the same transaction. The index records the claim and the fact
separately.

## The interface

Ten screens, every one on the path from a proposal to a payment: landing,
community directory, community home, proposal detail, new proposal, the queue,
treasury, delegates, your membership, operations.

The design is a civic register. Cool paper, one authoritative ink, and colour
used only where money or authority is involved. `web/src/styles/tokens.css`
has it as code.

Four rules in that file do most of the work.

- **Money is never a decoration.** One strong colour marks funds leaving a
  treasury and nothing else earns it.
- **Every number is comparable.** Tabular figures throughout, aligned on the
  decimal, so a balance does not shift as it changes.
- **Countdowns are honest.** A queue timer shows ledgers, which is what the
  contract counts, and labels the time beside it an estimate.
- **Empty states are told truthfully.** A community that has executed nothing
  says so. Turnout of four percent is shown as four percent.

Public pages read the index and never the chain. What is left is the connected
member's own state and the actions they take, both of which have to be current
to the ledger rather than to the last time an indexer ran.

## What is not built

Named plainly, because the roadmap has six phases and this is not all of them.

- No audit. The first release restricts proposals to treasury payments and
  governance parameter changes for that reason, and unrestricted contract calls
  should arrive after an audit rather than before one.
- Nothing is deployed to mainnet, and nothing here should hold real money until
  it has been.
- The interface does not yet let a member queue or execute a proposal, or pull
  the guardian's brake. Both are permissionless contract calls that work today
  from the CLI or the seed script; neither has a button yet.
