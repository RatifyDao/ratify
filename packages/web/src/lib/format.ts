/**
 * How RatifyDAO writes numbers, addresses and time.
 *
 * Three rules, from the design system, and every one of them is a decision
 * about honesty rather than taste.
 *
 * Every number is comparable, so figures are tabular and grouped the same way
 * everywhere.
 *
 * Countdowns are honest, so a ledger count is a ledger count and the time
 * beside it is labelled an estimate. Ledger time drifts and pretending
 * otherwise puts a false precision on the one number a member is relying on.
 *
 * Empty states are told truthfully, so turnout of four percent reads as four
 * percent and a community that has executed nothing says nothing.
 */

/** Stellar's ledgers close about every five seconds. About. */
export const LEDGER_SECONDS = 5;

const AMOUNT_DECIMALS = 7;

/**
 * Formats a contract amount, which arrives as a string of stroops.
 *
 * Kept as a string the whole way. A treasury balance that exceeds what a
 * double can hold exactly is not a hypothetical, and rounding one to make it
 * fit is not a thing a ledger does.
 */
export function amount(raw: string | number | null | undefined, decimals = AMOUNT_DECIMALS): string {
  if (raw === null || raw === undefined || raw === "") return "0";
  const text = String(raw);
  const negative = text.startsWith("-");
  const digits = (negative ? text.slice(1) : text).replace(/\D/g, "") || "0";

  const padded = digits.padStart(decimals + 1, "0");
  const whole = padded.slice(0, padded.length - decimals);
  const fraction = decimals > 0 ? padded.slice(padded.length - decimals) : "";

  const grouped = whole.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
  const trimmed = fraction.replace(/0+$/, "");

  return `${negative ? "-" : ""}${grouped}${trimmed ? `.${trimmed}` : ""}`;
}

/** A plain count, grouped. Voting power, members, proposals. */
export function count(raw: string | number | null | undefined): string {
  if (raw === null || raw === undefined || raw === "") return "0";
  return String(raw).replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

/**
 * A share, written as a percentage.
 *
 * Small shares are shown small. A turnout of four percent is a fact about the
 * community, and rounding it up to something more comfortable would be the
 * interface lying on the community's behalf.
 */
export function percent(part: number | string, whole: number | string): string {
  const a = Number(part);
  const b = Number(whole);
  if (!Number.isFinite(a) || !Number.isFinite(b) || b === 0) return "—";
  const value = (a / b) * 100;
  if (value > 0 && value < 1) return `${value.toFixed(1)}%`;
  return `${Math.round(value)}%`;
}

/** A rate the registry keeps in hundredths of a percent. */
export function bps(value: number | null | undefined): string {
  if (value === null || value === undefined) return "—";
  const asPercent = value / 100;
  if (asPercent > 0 && asPercent < 1) return `${asPercent.toFixed(1)}%`;
  return `${Math.round(asPercent)}%`;
}

/**
 * An address, shortened in the middle, never wrapped.
 *
 * The middle goes because the ends are what a member checks. Stolla filed an
 * issue against itself about addresses breaking its layout; this is the fix
 * rather than the ticket.
 */
export function shortAddress(address: string | null | undefined, keep = 6): string {
  if (!address) return "—";
  if (address.length <= keep * 2 + 3) return address;
  return `${address.slice(0, keep)}…${address.slice(-keep)}`;
}

/** A proposal or operation identifier, shortened the same way. */
export function shortHash(hash: string | null | undefined, keep = 8): string {
  if (!hash) return "—";
  if (hash.length <= keep * 2 + 3) return hash;
  return `${hash.slice(0, keep)}…${hash.slice(-keep)}`;
}

/** A ledger close time, written plainly. */
export function when(seconds: number | null | undefined): string {
  if (!seconds) return "—";
  return new Date(seconds * 1000).toLocaleString("en-GB", {
    day: "numeric",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function date(seconds: number | null | undefined): string {
  if (!seconds) return "—";
  return new Date(seconds * 1000).toLocaleDateString("en-GB", {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}

/**
 * A countdown, in ledgers, with an estimate of the time beside it.
 *
 * Both halves are returned separately so the interface can print the ledger
 * count as fact and the time as what it is. A queue timer that shows only
 * "about two days" is asking a member to trust an assumption about block
 * production that nobody controls.
 */
export function countdown(ledgers: number): { ledgers: string; estimate: string } {
  if (ledgers <= 0) return { ledgers: "0", estimate: "ready now" };

  const seconds = ledgers * LEDGER_SECONDS;
  const days = Math.floor(seconds / 86_400);
  const hours = Math.floor((seconds % 86_400) / 3_600);
  const minutes = Math.floor((seconds % 3_600) / 60);

  let estimate: string;
  if (days > 0) estimate = hours > 0 ? `about ${days}d ${hours}h` : `about ${days}d`;
  else if (hours > 0) estimate = minutes > 0 ? `about ${hours}h ${minutes}m` : `about ${hours}h`;
  else estimate = `about ${Math.max(minutes, 1)}m`;

  return { ledgers: count(ledgers), estimate };
}

/** A span of ledgers, as a duration. Used for delays and terms. */
export function duration(ledgers: number): string {
  if (!ledgers) return "none";
  const seconds = ledgers * LEDGER_SECONDS;
  const days = seconds / 86_400;
  if (days >= 1) return `${count(Math.round(days))} day${Math.round(days) === 1 ? "" : "s"}`;
  const hours = seconds / 3_600;
  if (hours >= 1) return `${Math.round(hours)} hour${Math.round(hours) === 1 ? "" : "s"}`;
  return `${Math.max(Math.round(seconds / 60), 1)} minutes`;
}

/**
 * The state of a proposal, derived from what the index holds.
 *
 * The index records what happened; the state at any moment also depends on
 * where the ledger has got to, which is why it is computed here rather than
 * stored.
 */
export type ProposalState =
  | "pending"
  | "active"
  | "defeated"
  | "succeeded"
  | "queued"
  | "executed"
  | "cancelled";

export function proposalState(
  proposal: {
    snapshot: number;
    deadline: number;
    executed_ledger: number | null;
    cancelled_ledger: number | null;
    queued_ledger: number | null;
  },
  tally: { forVotes: bigint; againstVotes: bigint; abstainVotes: bigint },
  quorum: bigint,
  ledger: number,
): ProposalState {
  if (proposal.executed_ledger) return "executed";
  if (proposal.cancelled_ledger) return "cancelled";
  if (proposal.queued_ledger) return "queued";
  if (ledger <= proposal.snapshot) return "pending";
  if (ledger <= proposal.deadline) return "active";

  const reached = tally.forVotes + tally.abstainVotes >= quorum;
  const carried = tally.forVotes > tally.againstVotes;
  return reached && carried ? "succeeded" : "defeated";
}

/** The chip colour a state earns. Only executed payment states get outflow. */
export function stateTone(state: ProposalState): string {
  switch (state) {
    case "executed":
      return "chip--outflow";
    case "succeeded":
      return "chip--positive";
    case "queued":
      return "chip--authority";
    case "active":
      return "chip--authority";
    case "pending":
      return "chip--quiet";
    default:
      return "chip--quiet";
  }
}

export function stateLabel(state: ProposalState): string {
  switch (state) {
    case "pending":
      return "Voting not open";
    case "active":
      return "Voting open";
    case "defeated":
      return "Defeated";
    case "succeeded":
      return "Approved";
    case "queued":
      return "In the queue";
    case "executed":
      return "Executed";
    case "cancelled":
      return "Stopped";
  }
}
