# Changelog

All notable changes to RatifyDAO are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions follow [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

### Added
- `packages/shared` — single source of truth for types, addresses, constants,
  and environment validation shared between the indexer and the web app.
- `packages/indexer/src/logger.ts` — structured, levelled JSON logger.
- `packages/indexer/src/migrations.ts` — versioned, forward-only DB migrations.
- `packages/indexer/src/retry.ts` — exponential back-off with jitter for RPC calls.
- `packages/indexer/src/health.ts` — `/health` and `/ready` HTTP endpoints.
- `packages/indexer/src/rate-limit.ts` — token-bucket rate limiter for RPC calls.
- `packages/web/src/components/error-boundary.tsx` — React error boundary with
  recovery UI.
- Address validation on all `read()` and `send()` calls in `chain.ts`.
- Contract method name constants in `shared/src/contracts.ts`.
- Proposal state derivation in `shared/src/proposal-state.ts`.
- Monorepo structure under `packages/` with a root `package.json` workspace.

### Changed
- Root folder renamed from `assent` to `ratify-dao`.
- All contract packages renamed from `assent-*` to `ratify-*`.
- All environment variables renamed from `ASSENT_*` to `RATIFY_*`.
- Brand renamed from Assent to RatifyDAO throughout.

---

## [0.1.0] — 2026-09-09

### Added
- Seven Soroban contracts: `ratify-treasury`, `ratify-timelock`,
  `ratify-membership`, `ratify-weight-rule`, `ratify-governor`,
  `ratify-delegate-registry`, `ratify-factory`.
- TypeScript indexer consuming contract events into SQLite.
- Next.js 14 web app with ten governance screens.
- Testnet deployment with a payment made by vote through the full path.
- 145 contract tests covering the full governance lifecycle.
