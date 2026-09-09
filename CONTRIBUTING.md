# Contributing to RatifyDAO

Thank you for taking the time to contribute. This document covers the full
process from opening an issue to merging a pull request.

---

## Before you start

- Check [open issues](../../issues) to see if your bug or idea is already being
  tracked.
- For significant changes — new contracts, new screens, protocol changes — open
  an issue first and discuss the approach before writing code. A rejected PR
  after a large investment of time is frustrating for everyone.
- For security vulnerabilities, see [SECURITY.md](SECURITY.md). Do **not**
  open a public issue.

---

## Development setup

```bash
# Clone and install
git clone https://github.com/RatifyDao/ratify.git
cd ratify
npm install          # installs all workspace packages

# Contracts
stellar contract build
cargo test --workspace

# Indexer
cd packages/indexer
cp ../../.env.example ../../.env   # fill in RATIFY_FACTORY_ID
npm start

# Web
cd packages/web
npm run dev          # http://localhost:3000
```

### Windows

Run `bash scripts/windows-dev-setup.sh` once after cloning. See the script
header for why this is needed on windows-gnu.

---

## Workflow

1. Fork the repository and create a branch from `main`.
2. Branch names: `fix/short-description`, `feat/short-description`,
   `chore/short-description`.
3. Make your changes. Keep commits focused — one logical change per commit.
4. Run the relevant checks before opening a PR:

   | Layer | Command |
   |---|---|
   | Contracts | `cargo test --workspace` |
   | Indexer types | `npm run typecheck --workspace=packages/indexer` |
   | Web types | `npm run typecheck --workspace=packages/web` |
   | Web lint | `npm run lint --workspace=packages/web` |
   | Full suite | `bash scripts/test.sh` |

5. Open a pull request against `main`. Fill in the PR template.
6. A maintainer will review within a few days. Address feedback with new
   commits — do not force-push a branch under review.

---

## Commit style

```
type(scope): short description

Optional longer explanation.
```

Types: `feat`, `fix`, `refactor`, `test`, `chore`, `docs`.
Scopes: `contracts`, `indexer`, `web`, `shared`, `ci`.

Example: `feat(contracts): add per-proposal quorum override`

---

## Contract changes

Changes to Soroban contracts need extra care.

- Every behaviour change must have a test in the contract's `test.rs`.
- If the change affects the event schema, update `packages/indexer/src/schema.ts`
  and add a migration in `packages/indexer/src/migrations.ts`.
- If the change affects types shared between packages, update
  `packages/shared/src/types.ts`.
- Deployed contracts on testnet are **not** automatically updated. Note the
  change in `DEPLOYMENT.md` so operators know to redeploy.

---

## Questions

Open a [discussion](../../discussions) rather than an issue for general
questions, design ideas, or anything that is not a bug or a concrete feature
request.
