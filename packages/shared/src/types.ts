/**
 * Canonical domain types shared by the indexer and the web app.
 *
 * A type that appears in both packages belongs here. If it drifts between
 * packages it becomes two types that happen to have the same name, which is
 * a bug waiting to surface at runtime.
 */

// ─── Governance ───────────────────────────────────────────────────────────────

/** Every state a proposal can be in, matching the on-chain enum. */
export type ProposalState =
  | "pending"
  | "active"
  | "succeeded"
  | "defeated"
  | "expired"
  | "queued"
  | "executed"
  | "cancelled";

/** How a member voted on a proposal. */
export type Ballot = "for" | "against" | "abstain";

/** Numeric vote type as the contract encodes it → canonical Ballot. */
export const VOTE_TYPE: Record<number, Ballot> = {
  0: "against",
  1: "for",
  2: "abstain",
};

/** Canonical Ballot → numeric vote type for transaction building. */
export const VOTE_TYPE_NUM: Record<Ballot, number> = {
  against: 0,
  for: 1,
  abstain: 2,
};

// ─── Community ────────────────────────────────────────────────────────────────

/** One entry in the factory's register. */
export interface Community {
  governor: string;
  idx: number;
  name: string;
  membership: string;
  weightRule: string;
  timelock: string;
  treasury: string;
  registry: string;
  founder: string;
  deployedAt: number;
}

/** A proposal row as stored in the index. */
export interface Proposal {
  id: string;
  governor: string;
  proposer: string;
  description: string;
  /** JSON-encoded list of on-chain actions. */
  actions: string;
  snapshot: number;
  deadline: number;
  createdLedger: number;
  createdAt: number;
  createdTx: string;
  queuedLedger: number | null;
  queuedTx: string | null;
  eta: number | null;
  executedLedger: number | null;
  executedTx: string | null;
  cancelledLedger: number | null;
  cancelledTx: string | null;
  cancelReason: string | null;
  cancelledBy: string | null;
}

/** Derived vote totals for a proposal. */
export interface Tally {
  forVotes: bigint;
  againstVotes: bigint;
  abstainVotes: bigint;
  voters: number;
}

/** One vote as stored in the index. */
export interface Vote {
  proposalId: string;
  voter: string;
  voteType: number;
  ballot: Ballot;
  weight: string;
  reason: string;
  ledger: number;
  ts: number;
  tx: string;
}

// ─── Treasury ─────────────────────────────────────────────────────────────────

/** A treasury outflow — money leaving. */
export interface Outflow {
  treasury: string;
  idx: number;
  governor: string;
  asset: string;
  recipient: string;
  amount: string;
  proposalId: string;
  ledger: number;
  ts: number;
  tx: string;
}

/** A spending policy for one asset. */
export interface Policy {
  treasury: string;
  asset: string;
  perPaymentCap: string;
  windowCap: string;
  windowLedgers: number;
  removed: boolean;
  ledger: number;
}

// ─── Membership ───────────────────────────────────────────────────────────────

/** A voting-power grant. */
export interface Grant {
  membership: string;
  account: string;
  delegatee: string;
  units: string;
  expiresAt: number;
  grantedAt: number;
  state: "live" | "lapsed" | "withdrawn";
  ledger: number;
}

/** Aggregate delegate view used by the delegates page. */
export interface Delegate {
  account: string;
  power: bigint;
  heads: number;
  soonestLapse: number;
  eligible: number;
  voted: number;
  contestedEligible: number;
  contestedVoted: number;
}

// ─── Queue ────────────────────────────────────────────────────────────────────

export type QueueItemState = "waiting" | "ready" | "executed" | "cancelled";

export interface QueueItem {
  operationId: string;
  governor: string;
  timelock: string;
  target: string;
  fn: string;
  scheduledLedger: number;
  readyAt: number;
  state: QueueItemState;
  executedLedger: number | null;
  cancelledLedger: number | null;
  cancelReason: string | null;
  cancelledBy: string | null;
  tx: string;
}
