import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
// src/ → packages/indexer/ → packages/ → ratify-dao/ (repo root)
const root = resolve(here, "../../..");

/**
 * Reads `.env` from the repository root, if there is one.
 *
 * Deliberately small. Anything that needs more than this belongs in the
 * process environment, where a deployment can set it.
 */
function loadDotEnv(): void {
  try {
    const text = readFileSync(resolve(root, ".env"), "utf8");
    for (const line of text.split("\n")) {
      const trimmed = line.trim();
      if (!trimmed || trimmed.startsWith("#")) continue;
      const eq = trimmed.indexOf("=");
      if (eq === -1) continue;
      const key = trimmed.slice(0, eq).trim();
      const value = trimmed.slice(eq + 1).trim();
      if (!(key in process.env)) process.env[key] = value;
    }
  } catch {
    // No .env. The defaults below are testnet, which is where this runs.
  }
}

loadDotEnv();

export const config = {
  rpcUrl: process.env.RATIFY_RPC_URL ?? "https://soroban-testnet.stellar.org",
  networkPassphrase:
    process.env.RATIFY_NETWORK_PASSPHRASE ?? "Test SDF Network ; September 2015",
  /** The factory whose register defines which communities exist. */
  factoryId: process.env.RATIFY_FACTORY_ID ?? "",
  dbPath: resolve(root, process.env.RATIFY_DB_PATH ?? "./data/ratify.db"),
  pollMs: Number(process.env.RATIFY_POLL_MS ?? 5000),
  /**
   * Ledgers to ask for in one request.
   *
   * Soroban RPC caps the range it will serve. Asking for more than it will
   * give produces an error rather than a truncated answer, so this stays
   * conservative and the loop simply runs more often.
   */
  ledgerWindow: 2000,
};

export const root_dir = root;
