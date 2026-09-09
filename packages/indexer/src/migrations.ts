/**
 * Improvement 2 — Versioned DB migrations.
 *
 * Every schema change is a numbered, append-only migration. Migrations run
 * forward only — never destructive, never re-run. The current version is
 * stored in a `schema_version` meta row, so the indexer always knows exactly
 * what shape the database is in.
 *
 * To add a schema change:
 *   1. Append a new entry to MIGRATIONS with the next version number.
 *   2. Never edit an existing entry — write a corrective migration instead.
 *
 * Usage:
 *   import { runMigrations } from "./migrations.js";
 *   runMigrations(db);   // call once after open(), before anything else
 */

import type { Database } from "better-sqlite3";
import { log } from "./logger.js";

interface Migration {
  version: number;
  description: string;
  up: string;
}

/**
 * The ordered list of all schema migrations.
 *
 * Version 0 is the baseline schema that SCHEMA already creates (via
 * `CREATE TABLE IF NOT EXISTS`). Migrations here are additive changes made
 * after that baseline shipped.
 */
const MIGRATIONS: Migration[] = [
  {
    version: 1,
    description: "Add schema_version row to meta if not present",
    up: `INSERT OR IGNORE INTO meta (key, value) VALUES ('schema_version', '0');`,
  },
  {
    version: 2,
    description: "Add weight column to votes table",
    up: `ALTER TABLE votes ADD COLUMN IF NOT EXISTS raw_weight TEXT NOT NULL DEFAULT '';`,
  },
  {
    version: 3,
    description: "Add contested_margin_bps to communities table",
    up: `ALTER TABLE communities ADD COLUMN IF NOT EXISTS contested_margin_bps INTEGER NOT NULL DEFAULT 2000;`,
  },
];

/** Applies all pending migrations and records the new version. */
export function runMigrations(db: Database): void {
  const currentVersion = (() => {
    try {
      const row = db
        .prepare(`SELECT value FROM meta WHERE key = 'schema_version'`)
        .get() as { value: string } | undefined;
      return row ? Number(row.value) : 0;
    } catch {
      return 0;
    }
  })();

  const pending = MIGRATIONS.filter((m) => m.version > currentVersion);
  if (pending.length === 0) return;

  log.info("Running DB migrations", { from: currentVersion, count: pending.length });

  const apply = db.transaction(() => {
    for (const migration of pending) {
      log.info(`Migration ${migration.version}: ${migration.description}`);
      db.exec(migration.up);
      db.prepare(`INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?)`).run(
        String(migration.version),
      );
    }
  });

  apply();
  log.info("Migrations complete", { version: pending.at(-1)!.version });
}
