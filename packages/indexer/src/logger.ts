/**
 * Improvement 1 — Structured logger.
 *
 * Replaces bare console.log calls with levelled, timestamped, prefixed
 * output. In production the level is read from RATIFY_LOG_LEVEL; the default
 * is "info". A structured format makes it grep-able and ingest-able by any
 * log aggregator without post-processing.
 *
 * Usage:
 *   import { log } from "./logger.js";
 *   log.info("Following factory", { address: config.factoryId });
 *   log.warn("RPC slow", { ms: elapsed });
 *   log.error("Handler threw", { error: e.message });
 */

export type LogLevel = "debug" | "info" | "warn" | "error";

const LEVELS: Record<LogLevel, number> = { debug: 0, info: 1, warn: 2, error: 3 };

function activeLevel(): LogLevel {
  const raw = (process.env.RATIFY_LOG_LEVEL ?? "info").toLowerCase();
  return (raw in LEVELS ? raw : "info") as LogLevel;
}

function emit(level: LogLevel, message: string, context?: Record<string, unknown>): void {
  if (LEVELS[level] < LEVELS[activeLevel()]) return;

  const entry: Record<string, unknown> = {
    ts: new Date().toISOString(),
    level,
    msg: message,
    ...context,
  };

  const line = JSON.stringify(entry);

  if (level === "error" || level === "warn") {
    process.stderr.write(line + "\n");
  } else {
    process.stdout.write(line + "\n");
  }
}

export const log = {
  debug: (msg: string, ctx?: Record<string, unknown>) => emit("debug", msg, ctx),
  info:  (msg: string, ctx?: Record<string, unknown>) => emit("info",  msg, ctx),
  warn:  (msg: string, ctx?: Record<string, unknown>) => emit("warn",  msg, ctx),
  error: (msg: string, ctx?: Record<string, unknown>) => emit("error", msg, ctx),
};
