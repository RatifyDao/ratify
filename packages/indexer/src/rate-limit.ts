/**
 * Improvement 10 — Token-bucket rate limiter for RPC calls.
 *
 * Soroban RPC providers enforce per-IP rate limits. Bursting requests during
 * a large replay causes 429s, which the retry logic then retries, making the
 * problem worse. A token bucket prevents the burst at the source.
 *
 * The bucket refills at `ratePerSecond` tokens per second, up to `capacity`.
 * Each RPC call acquires one token before it runs. If the bucket is empty,
 * the call waits until a token is available.
 *
 * Usage:
 *   import { RateLimiter } from "./rate-limit.js";
 *
 *   const limiter = new RateLimiter({ ratePerSecond: 10, capacity: 20 });
 *
 *   // Wraps any async call:
 *   const result = await limiter.run(() => server.getLatestLedger());
 */

export interface RateLimiterOptions {
  /** How many tokens are added per second. */
  ratePerSecond: number;
  /** Maximum tokens the bucket can hold. Defaults to ratePerSecond * 2. */
  capacity?: number;
}

export class RateLimiter {
  private tokens: number;
  private readonly capacity: number;
  private readonly ratePerSecond: number;
  private lastRefill: number;

  constructor(options: RateLimiterOptions) {
    this.ratePerSecond = options.ratePerSecond;
    this.capacity = options.capacity ?? options.ratePerSecond * 2;
    this.tokens = this.capacity;
    this.lastRefill = Date.now();
  }

  /** Acquires a token, waiting if necessary, then runs `fn`. */
  async run<T>(fn: () => Promise<T>): Promise<T> {
    await this.acquire();
    return fn();
  }

  private async acquire(): Promise<void> {
    while (true) {
      this.refill();
      if (this.tokens >= 1) {
        this.tokens -= 1;
        return;
      }
      // Wait for the next token to become available.
      const waitMs = Math.ceil((1 / this.ratePerSecond) * 1000);
      await sleep(waitMs);
    }
  }

  private refill(): void {
    const now = Date.now();
    const elapsed = (now - this.lastRefill) / 1000;
    this.tokens = Math.min(this.capacity, this.tokens + elapsed * this.ratePerSecond);
    this.lastRefill = now;
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
