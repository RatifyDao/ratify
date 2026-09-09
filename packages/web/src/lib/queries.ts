import { row, rows, meta } from "./index";

/**
 * Every question the ten screens ask of the index.
 *
 * Kept in one place because most of them are the same question from a
 * different angle, and because a figure that appears on two pages should be
 * counted the same way on both. Turnout on the directory and turnout on a
 * community's own page are the same number or the interface is lying on one
 * of them.
 */

export interface Community {
  governor: string;
  idx: number;
  name: string;
  membership: string;
  weight_rule: string;
  timelock: string;
  treasury: string;
  registry: string;
  founder: string;
  deployed_at: number;
}

export interface Proposal {
  id: string;
  governor: string;
  proposer: string;
  description: string;
  actions: string;
  snapshot: number;
  deadline: number;
  created_ledger: number;
  created_at: number;
  created_tx: string;
  queued_ledger: number | null;
  queued_tx: string | null;
  eta: number | null;
  executed_ledger: number | null;
  executed_tx: string | null;
  cancelled_ledger: number | null;
  cancelled_tx: string | null;
  cancel_reason: string | null;
  cancelled_by: string | null;
}

export interface Vote {
  proposal_id: string;
  voter: string;
  vote_type: number;
  weight: string;
  reason: string;
  ledger: number;
  ts: number;
  tx: string;
}

export interface Outflow {
  treasury: string;
  idx: number;
  governor: string;
  asset: string;
  recipient: string;
  amount: string;
  proposal_id: string;
  ledger: number;
  ts: number;
  tx: string;
}

export interface QueueItem {
  operation_id: string;
  governor: string;
  timelock: string;
  target: string;
  fn: string;
  scheduled_ledger: number;
  ready_at: number;
  state: string;
  executed_ledger: number | null;
  cancelled_ledger: number | null;
  cancel_reason: string | null;
  cancelled_by: string | null;
  tx: string;
}

export interface Policy {
  treasury: string;
  asset: string;
  per_payment_cap: string;
  window_cap: string;
  window_ledgers: number;
  removed: number;
  ledger: number;
}

export interface Grant {
  membership: string;
  account: string;
  delegatee: string;
  units: string;
  expires_at: number;
  granted_at: number;
  state: string;
  ledger: number;
}

/** The ledger the index has read up to. Every countdown is measured from it. */
export function currentLedger(): number {
  const last = meta("last_ledger");
  return last ? Number(last) : 0;
}

// ################## COMMUNITIES ##################

export function communities(): Community[] {
  return rows<Community>("SELECT * FROM communities ORDER BY idx ASC");
}

export function community(governor: string): Community | null {
  return row<Community>("SELECT * FROM communities WHERE governor = ?", governor);
}

// ################## PROPOSALS ##################

export function proposals(governor: string, limit = 100): Proposal[] {
  return rows<Proposal>(
    "SELECT * FROM proposals WHERE governor = ? ORDER BY created_ledger DESC LIMIT ?",
    governor,
    limit,
  );
}

export function proposal(id: string): Proposal | null {
  return row<Proposal>("SELECT * FROM proposals WHERE id = ?", id);
}

export function votes(proposalId: string): Vote[] {
  return rows<Vote>(
    "SELECT * FROM votes WHERE proposal_id = ? ORDER BY ledger ASC",
    proposalId,
  );
}

/**
 * The tally on a proposal.
 *
 * Summed from the votes as recorded rather than read back from the contract,
 * because the index has to be able to show a proposal from before an RPC
 * provider's retention window, and the votes are what it kept.
 */
export function tally(proposalId: string): {
  forVotes: bigint;
  againstVotes: bigint;
  abstainVotes: bigint;
  voters: number;
} {
  const all = votes(proposalId);
  let forVotes = 0n;
  let againstVotes = 0n;
  let abstainVotes = 0n;
  for (const vote of all) {
    const weight = BigInt(vote.weight || "0");
    if (vote.vote_type === 1) forVotes += weight;
    else if (vote.vote_type === 0) againstVotes += weight;
    else abstainVotes += weight;
  }
  return { forVotes, againstVotes, abstainVotes, voters: all.length };
}

// ################## THE QUEUE ##################

export function queue(timelock: string): QueueItem[] {
  return rows<QueueItem>(
    "SELECT * FROM queue_items WHERE timelock = ? ORDER BY ready_at ASC",
    timelock,
  );
}

export function waitingQueue(timelock: string): QueueItem[] {
  return queue(timelock).filter((item) => item.state === "waiting");
}

// ################## THE MONEY ##################

export function outflows(treasury: string, limit = 200): Outflow[] {
  return rows<Outflow>(
    "SELECT * FROM outflows WHERE treasury = ? ORDER BY ledger DESC LIMIT ?",
    treasury,
    limit,
  );
}

export function policies(treasury: string): Policy[] {
  return rows<Policy>(
    "SELECT * FROM policies WHERE treasury = ? AND removed = 0 ORDER BY asset ASC",
    treasury,
  );
}

/**
 * What has left the treasury inside an asset's current rolling window.
 *
 * Computed the same way the contract computes it: payments whose ledger is
 * still inside the window count, and the rest have aged out.
 */
export function windowSpent(treasury: string, policy: Policy, ledger: number): bigint {
  const cutoff = Math.max(ledger - policy.window_ledgers, 0);
  const recent = rows<{ amount: string }>(
    "SELECT amount FROM outflows WHERE treasury = ? AND asset = ? AND ledger > ?",
    treasury,
    policy.asset,
    cutoff,
  );
  return recent.reduce((total, o) => total + BigInt(o.amount || "0"), 0n);
}

export function paidOut(treasury: string): bigint {
  const all = rows<{ amount: string }>(
    "SELECT amount FROM outflows WHERE treasury = ?",
    treasury,
  );
  return all.reduce((total, o) => total + BigInt(o.amount || "0"), 0n);
}

// ################## MEMBERSHIP AND DELEGATES ##################

export function grants(membership: string): Grant[] {
  return rows<Grant>("SELECT * FROM grants WHERE membership = ?", membership);
}

export function grantFor(membership: string, account: string): Grant | null {
  return row<Grant>(
    "SELECT * FROM grants WHERE membership = ? AND account = ?",
    membership,
    account,
  );
}

export function memberCount(membership: string): number {
  return (
    row<{ n: number }>(
      "SELECT COUNT(*) AS n FROM members WHERE membership = ? AND tokens > 0",
      membership,
    )?.n ?? 0
  );
}

export function tokensHeld(membership: string, account: string): number {
  return (
    row<{ tokens: number }>(
      "SELECT tokens FROM members WHERE membership = ? AND account = ?",
      membership,
      account,
    )?.tokens ?? 0
  );
}

/**
 * A delegate, as the delegates page shows them.
 *
 * Power and heads come from live grants; the record comes from settlements
 * the registry has made. Both are on chain and neither can be edited by the
 * delegate or by this interface.
 */
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

export function delegates(c: Community, ledger: number): Delegate[] {
  const live = grants(c.membership).filter(
    (grant) => grant.state === "live" && grant.expires_at > ledger,
  );

  const byDelegate = new Map<string, Delegate>();
  for (const grant of live) {
    const existing = byDelegate.get(grant.delegatee) ?? {
      account: grant.delegatee,
      power: 0n,
      heads: 0,
      soonestLapse: Number.MAX_SAFE_INTEGER,
      eligible: 0,
      voted: 0,
      contestedEligible: 0,
      contestedVoted: 0,
    };
    existing.power += BigInt(grant.units || "0");
    existing.heads += 1;
    existing.soonestLapse = Math.min(existing.soonestLapse, grant.expires_at);
    byDelegate.set(grant.delegatee, existing);
  }

  for (const delegate of byDelegate.values()) {
    const settled = rows<{
      turned_up: number;
      contested: number;
    }>(
      "SELECT turned_up, contested FROM settlements WHERE registry = ? AND account = ?",
      c.registry,
      delegate.account,
    );
    for (const s of settled) {
      delegate.eligible += 1;
      if (s.turned_up) delegate.voted += 1;
      if (s.contested) {
        delegate.contestedEligible += 1;
        if (s.turned_up) delegate.contestedVoted += 1;
      }
    }
  }

  return [...byDelegate.values()].sort((a, b) => {
    const rateA = a.eligible ? a.voted / a.eligible : -1;
    const rateB = b.eligible ? b.voted / b.eligible : -1;
    if (rateA !== rateB) return rateB - rateA;
    return a.power > b.power ? -1 : a.power < b.power ? 1 : 0;
  });
}

// ################## TURNOUT ##################

/**
 * Turnout over the last `n` closed proposals, as a share of live voting power.
 *
 * The honest measure of whether a community is alive, which is why the
 * directory prints it beside every community whether it flatters them or not.
 * A community with no closed proposals returns null, because turnout out of
 * nothing is unknown rather than zero.
 */
export function turnout(c: Community, ledger: number, n = 5): number | null {
  const closed = proposals(c.governor, 50).filter((p) => p.deadline < ledger).slice(0, n);
  if (closed.length === 0) return null;

  const live = liveVotingPower(c, ledger);
  if (live === 0n) return null;

  let total = 0;
  for (const p of closed) {
    const t = tally(p.id);
    const cast = t.forVotes + t.againstVotes + t.abstainVotes;
    total += Number(cast) / Number(live);
  }
  return total / closed.length;
}

/** Voting power across the community that has not lapsed. */
export function liveVotingPower(c: Community, ledger: number): bigint {
  return grants(c.membership)
    .filter((grant) => grant.state === "live" && grant.expires_at > ledger)
    .reduce((total, grant) => total + BigInt(grant.units || "0"), 0n);
}

export function executedCount(governor: string): number {
  return (
    row<{ n: number }>(
      "SELECT COUNT(*) AS n FROM proposals WHERE governor = ? AND executed_ledger IS NOT NULL",
      governor,
    )?.n ?? 0
  );
}

// ################## THE LANDING PAGE ##################

/**
 * The numbers on the front page.
 *
 * Live, counted from the index, and shown small when they are small. Stolla
 * labelled a hardcoded panel as live and had to file an issue against itself
 * to correct it; the fix is not to have a panel that can be hardcoded.
 */
export function headline(ledger: number): {
  communities: number;
  executed: number;
  paidOut: number;
  medianTurnout: number | null;
} {
  const all = communities();
  const executed = all.reduce((total, c) => total + executedCount(c.governor), 0);
  const payments = rows<{ n: number }>("SELECT COUNT(*) AS n FROM outflows")[0]?.n ?? 0;

  const turnouts = all
    .map((c) => turnout(c, ledger))
    .filter((t): t is number => t !== null)
    .sort((a, b) => a - b);

  const median =
    turnouts.length === 0
      ? null
      : turnouts.length % 2 === 1
        ? (turnouts[(turnouts.length - 1) / 2] as number)
        : (((turnouts[turnouts.length / 2 - 1] as number) +
            (turnouts[turnouts.length / 2] as number)) /
          2);

  return {
    communities: all.length,
    executed,
    paidOut: payments,
    medianTurnout: median,
  };
}
