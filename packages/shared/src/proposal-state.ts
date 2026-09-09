/**
 * Improvement 6 — Proposal state derivation.
 *
 * A single pure function that maps a proposal row to its current state,
 * given the current ledger. The same logic runs in the indexer (for the
 * `state` column it might write) and in the web app (for the badge on every
 * proposal card). One function means the two can never disagree.
 *
 * The order of the checks mirrors the on-chain state machine in the governor
 * contract exactly. If it changes there, it changes here, and nowhere else.
 */

import type { Proposal, ProposalState } from "./types.js";

/**
 * Derives the current state of a proposal at `ledger`.
 *
 * @param proposal  The raw proposal row from the index.
 * @param ledger    The current ledger sequence number.
 * @returns         The canonical ProposalState string.
 */
export function deriveProposalState(proposal: Proposal, ledger: number): ProposalState {
  if (proposal.cancelledLedger !== null) return "cancelled";
  if (proposal.executedLedger  !== null) return "executed";
  if (proposal.queuedLedger    !== null) return "queued";

  if (ledger < proposal.snapshot) return "pending";
  if (ledger <= proposal.deadline) return "active";

  // Voting has closed. Read the tally from whatever the caller has.
  // The governor marks Succeeded/Defeated at deadline+1, but the index may
  // not have processed that block yet, so we derive it here rather than
  // trusting a stored string.
  //
  // We cannot read the tally here without importing queries, which creates a
  // circular dependency. Return "expired" as a sentinel and let the caller
  // resolve against its tally if it needs Succeeded/Defeated.
  return "expired";
}

/**
 * Full state derivation when tally data is available.
 *
 * Pass the vote totals and quorum to get the precise Succeeded or Defeated
 * state instead of the "expired" sentinel above.
 */
export function deriveProposalStateFull(
  proposal: Proposal,
  ledger: number,
  tally: { forVotes: bigint; againstVotes: bigint; abstainVotes: bigint },
  quorum: bigint,
): ProposalState {
  const base = deriveProposalState(proposal, ledger);
  if (base !== "expired") return base;

  const totalFor     = tally.forVotes;
  const totalAgainst = tally.againstVotes;
  const totalAbstain = tally.abstainVotes;
  const participation = totalFor + totalAgainst + totalAbstain;

  if (participation < quorum) return "defeated";
  if (totalFor > totalAgainst) return "succeeded";
  return "defeated";
}

/** Maps a ProposalState to the CSS class name used for its badge. */
export function proposalStateBadgeClass(state: ProposalState): string {
  switch (state) {
    case "pending":   return "badge badge--neutral";
    case "active":    return "badge badge--authority";
    case "succeeded": return "badge badge--positive";
    case "defeated":  return "badge badge--muted";
    case "expired":   return "badge badge--muted";
    case "queued":    return "badge badge--caution";
    case "executed":  return "badge badge--positive";
    case "cancelled": return "badge badge--muted";
  }
}
