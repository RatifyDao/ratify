import Database from "better-sqlite3";
import { mkdirSync } from "node:fs";
import { dirname } from "node:path";

import { config } from "./config.js";
import { SCHEMA } from "./schema.js";

export type Db = Database.Database;

/** Opens the index, creating it if it is not there yet. */
export function open(path = config.dbPath): Db {
  mkdirSync(dirname(path), { recursive: true });
  const db = new Database(path);
  db.exec(SCHEMA);
  return db;
}

export function getMeta(db: Db, key: string): string | null {
  const row = db.prepare("SELECT value FROM meta WHERE key = ?").get(key) as
    | { value: string }
    | undefined;
  return row?.value ?? null;
}

export function setMeta(db: Db, key: string, value: string): void {
  db.prepare(
    "INSERT INTO meta (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
  ).run(key, value);
}

/**
 * Empties the index so it can be filled again from the ledger.
 *
 * The whole point of an index that is a read model: throwing it away costs
 * the time to replay and nothing else. `npm run reset` is this.
 */
export function reset(db: Db): void {
  const tables = [
    "meta",
    "communities",
    "proposals",
    "votes",
    "queue_items",
    "outflows",
    "deposits",
    "policies",
    "grants",
    "members",
    "settlements",
    "events",
  ];
  const wipe = db.transaction(() => {
    for (const table of tables) db.prepare(`DELETE FROM ${table}`).run();
  });
  wipe();
}
