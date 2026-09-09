import Database from "better-sqlite3";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

/**
 * Reading the index.
 *
 * The interface reads the index and never the chain, except for the connected
 * member's own state. Public pages stay fast and stay readable when an RPC
 * provider is slow, and a proposal from two years ago loads exactly like one
 * from this morning.
 *
 * Everything here is read only. Nothing the interface does writes to the
 * index; only the indexer does, and only from events.
 */

// Next.js cwd is packages/web; data/ lives at repo root
const path = resolve(
  process.cwd(),
  "../..",
  process.env.RATIFY_DB_PATH ?? "./data/ratify.db",
);

let handle: Database.Database | null = null;

/**
 * Opens the index, or returns null if there is not one yet.
 *
 * A missing index is a real state, not an error: the project has been cloned
 * and the indexer has not been run. Pages say so rather than failing.
 */
export function index(): Database.Database | null {
  if (handle) return handle;
  if (!existsSync(path)) return null;
  handle = new Database(path, { readonly: true, fileMustExist: true });
  return handle;
}

export function indexPath(): string {
  return path;
}

export function rows<T>(sql: string, ...params: unknown[]): T[] {
  const db = index();
  if (!db) return [];
  try {
    return db.prepare(sql).all(...(params as never[])) as T[];
  } catch {
    // A query against a table an older index does not have yet. Empty is the
    // honest answer, and the operations page shows the index's state.
    return [];
  }
}

export function row<T>(sql: string, ...params: unknown[]): T | null {
  const db = index();
  if (!db) return null;
  try {
    return (db.prepare(sql).get(...(params as never[])) as T) ?? null;
  } catch {
    return null;
  }
}

export function meta(key: string): string | null {
  return row<{ value: string }>("SELECT value FROM meta WHERE key = ?", key)?.value ?? null;
}

/**
 * How current the index is.
 *
 * Shown on the operations page and nowhere else. A member should not have to
 * think about indexer lag to read a proposal; that is the operator's problem,
 * and the reason the indexer exists at all.
 */
export function indexState(): {
  present: boolean;
  lastLedger: number | null;
  events: number;
  startedAt: number | null;
} {
  const db = index();
  if (!db) return { present: false, lastLedger: null, events: 0, startedAt: null };
  const last = meta("last_ledger");
  const started = meta("started_at");
  const count = row<{ n: number }>("SELECT COUNT(*) AS n FROM events");
  return {
    present: true,
    lastLedger: last ? Number(last) : null,
    events: count?.n ?? 0,
    startedAt: started ? Number(started) : null,
  };
}
