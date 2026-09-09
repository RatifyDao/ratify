/**
 * Protocol-level constants.
 *
 * Every magic number that appears in both the indexer and the web app belongs
 * here. A number that drifts between packages is a bug; a number defined once
 * is a fact.
 */

/** Stellar's native asset identifier as used in contract calls. */
export const NATIVE_ASSET = "native";

/** Hundredths of a percent in the whole — used for quorum and margin math. */
export const BPS = 10_000n;

/** Approximate seconds per Stellar ledger (5 s target). */
export const LEDGER_SECONDS = 5;

/** Converts a ledger count to approximate seconds. */
export function ledgersToSeconds(ledgers: number): number {
  return ledgers * LEDGER_SECONDS;
}

/** Converts a ledger count to a human-readable duration string. */
export function ledgersToDuration(ledgers: number): string {
  const seconds = ledgersToSeconds(ledgers);
  if (seconds < 60)   return `${seconds}s`;
  if (seconds < 3600) return `${Math.round(seconds / 60)}m`;
  if (seconds < 86400) return `${Math.round(seconds / 3600)}h`;
  return `${Math.round(seconds / 86400)}d`;
}

/** The maximum number of actions one proposal may carry (mirrors the contract). */
export const MAX_PROPOSAL_ACTIONS = 8;

/**
 * The longest a cancellation reason may be in bytes (mirrors the contract).
 * Used by the frontend to validate before submitting.
 */
export const MAX_REASON_BYTES = 512;

/**
 * The maximum membership term in ledgers (2 years at 5 s/ledger).
 * Mirrors `MAX_TERM_LEDGERS` in the membership contract.
 */
export const MAX_TERM_LEDGERS = 2 * 365 * 17_280;

/**
 * Formats a bigint token amount as a decimal string.
 *
 * Stellar uses 7 decimal places (1 XLM = 10_000_000 stroops).
 * Membership tokens use 0 decimals.
 */
export function formatAmount(raw: bigint | string, decimals = 7): string {
  const n = typeof raw === "string" ? BigInt(raw) : raw;
  if (decimals === 0) return n.toString();
  const factor = 10n ** BigInt(decimals);
  const whole = n / factor;
  const frac  = (n % factor).toString().padStart(decimals, "0").replace(/0+$/, "");
  return frac.length > 0 ? `${whole}.${frac}` : whole.toString();
}

/**
 * Parses a decimal string into a bigint with the given decimal precision.
 * Throws if the string is not a valid number.
 */
export function parseAmount(value: string, decimals = 7): bigint {
  const [whole = "0", frac = ""] = value.split(".");
  const fracPadded = frac.padEnd(decimals, "0").slice(0, decimals);
  return BigInt(whole) * 10n ** BigInt(decimals) + BigInt(fracPadded || "0");
}

/** Truncates a Stellar address for display: first 4 + … + last 4. */
export function shortAddress(address: string): string {
  if (address.length < 12) return address;
  return `${address.slice(0, 4)}…${address.slice(-4)}`;
}

/** Returns a bps value as a percentage string e.g. 2000 → "20%". */
export function bpsToPercent(bps: number): string {
  return `${(bps / 100).toFixed(bps % 100 === 0 ? 0 : 1)}%`;
}
