/**
 * Improvement 3 — RPC retry with exponential back-off.
 *
 * Wraps any async operation and retries it on failure, with increasing delays
 * and an optional jitter to spread thundering-herd reconnects. The indexer
 * runs continuously, so a single RPC blip should not bring it down.
 *
 * Usage:
 *   import { withRetry } from "./retry.js";
 *
 *   const result = await withRetry(
 *     () => server.getLatestLedger(),
 *     { label: "getLatestLedger" },
 *   );
 */

import { log } from "./logger.js";

export interface RetryOptions {
  /** Human label for log messages. */
  label: string;
  /** Maximum number of attempts (including the first). Default: 5. */
  maxAttempts?: number;
  /** Base delay in ms before the first retry. Default: 500. */
  baseDelayMs?: number;
  /** Maximum delay cap in ms. Default: 30_000. */
  maxDelayMs?: number;
  /** Add up to this many ms of random jitter. Default: 200. */
  jitterMs?: number;
  /** Return true to retry on this error; false to throw immediately. */
  retryIf?: (error: unknown) => boolean;
}

/**
 * Executes `fn`, retrying on failure with exponential back-off.
 * Throws the last error if all attempts are exhausted.
 */
export async function withRetry<T>(
  fn: () => Promise<T>,
  options: RetryOptions,
): Promise<T> {
  const {
    label,
    maxAttempts = 5,
    baseDelayMs = 500,
    maxDelayMs = 30_000,
    jitterMs = 200,
    retryIf = () => true,
  } = options;

  let lastError: unknown;

  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    try {
      return await fn();
    } catch (error) {
      lastError = error;

      if (!retryIf(error) || attempt === maxAttempts) {
        throw error;
      }

      const backoff = Math.min(baseDelayMs * 2 ** (attempt - 1), maxDelayMs);
      const jitter = Math.random() * jitterMs;
      const delay = Math.round(backoff + jitter);

      log.warn(`${label} failed, retrying`, {
        attempt,
        maxAttempts,
        delayMs: delay,
        error: (error as Error).message,
      });

      await sleep(delay);
    }
  }

  throw lastError;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
