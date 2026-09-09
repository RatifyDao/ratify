import { rpc, xdr } from "@stellar/stellar-sdk";

import { config } from "./config.js";
import { getMeta, open, reset, setMeta, type Db } from "./db.js";
import { decodeBody, topicAddress, topicName } from "./decode.js";
import {
  handle,
  loadRoles,
  reconcile,
  watchedContracts,
  type Event,
} from "./handlers.js";
import { log } from "./logger.js";
import { runMigrations } from "./migrations.js";
import { startHealthServer } from "./health.js";
import { withRetry } from "./retry.js";
import { RateLimiter } from "./rate-limit.js";
import { assertEnv } from "../../shared/src/env.js";

/**
 * The RatifyDAO indexer.
 *
 * Stolla scans contract events from the browser and deferred an indexer, so
 * its history is bounded by how long an RPC provider keeps events. It then
 * built a module to tell members their data may be stale, which measures the
 * decay rather than removing it.
 *
 * This is the removal. Events go into a durable store as they happen, the
 * interface reads the store, and the chain stays the source of truth. The
 * index is a read model: it can be thrown away and replayed, so losing it
 * costs the time to rebuild and nothing else.
 */

const LAST_LEDGER = "last_ledger";
const STARTED_AT = "started_at";

/** Soroban RPC takes at most five contracts in one filter. */
const CONTRACTS_PER_REQUEST = 5;

/** Improvement 10 — Rate limiter: max 10 RPC calls/s, burst up to 20. */
const limiter = new RateLimiter({ ratePerSecond: 10, capacity: 20 });

async function main(): Promise<void> {
  // Improvement 5 — Validate environment before doing anything else.
  assertEnv();

  const once = process.argv.includes("--once");
  const wipe = process.argv.includes("--reset");

  const db = open();

  // Improvement 2 — Run schema migrations before first query.
  runMigrations(db);

  if (wipe) {
    reset(db);
    log.info("Index emptied. It will be rebuilt from the ledger.");
  }

  // Improvement 4 — Health server.
  const health = startHealthServer();

  const server = new rpc.Server(config.rpcUrl, {
    allowHttp: config.rpcUrl.startsWith("http://"),
  });

  log.info("Following factory", { address: config.factoryId });
  log.info("Network", { rpc: config.rpcUrl });
  log.info("Index", { path: config.dbPath });

  if (!getMeta(db, STARTED_AT)) {
    setMeta(db, STARTED_AT, String(Math.floor(Date.now() / 1000)));
  }

  let stopping = false;
  process.on("SIGINT", () => {
    stopping = true;
    log.info("Stopping. The last ledger read is saved; the next run resumes there.");
  });

  do {
    try {
      const read = await step(db, server);
      if (read > 0) log.info("Events processed", { count: read });
      health.setReady(true);
    } catch (error) {
      log.error("Poll failed", { error: (error as Error).message });
    }
    if (once || stopping) break;
    await sleep(config.pollMs);
  } while (!stopping);

  health.close();
  db.close();
}

/**
 * Reads every event since the last pass and writes what it finds.
 *
 * Deliberately reads by ledger range rather than by cursor. A community
 * discovered halfway through a pass brings its own six contracts with it, and
 * a range can simply be asked again for the wider set. Every handler is safe
 * to run twice, so overlapping a pass costs nothing.
 */
async function step(db: Db, server: rpc.Server): Promise<number> {
  const latest = (await server.getLatestLedger()).sequence;
  const windowStart = Math.max(latest - config.ledgerWindow, 1);

  const contracts = watchedContracts(db, config.factoryId, windowStart);
  if (contracts.length === 0) return 0;

  const roles = loadRoles(db);
  const write = db.transaction((events: Event[]) => {
    for (const event of events) handle(db, event, roles, config.factoryId);
  });

  let total = 0;
  for (let i = 0; i < contracts.length; i += CONTRACTS_PER_REQUEST) {
    const chunk = contracts.slice(i, i + CONTRACTS_PER_REQUEST);
    const from = chunk.reduce(
      (earliest, contract) => Math.min(earliest, markFor(db, contract)),
      Number.MAX_SAFE_INTEGER,
    );

    // Improvement 3 + 10 — retry with back-off + rate limiting on every RPC call.
    total += await withRetry(
      () => limiter.run(() =>
        readChunk(
          server,
          chunk.map((contract) => contract.id),
          Math.max(Math.min(from, latest), 1),
          write,
        ),
      ),
      { label: "readChunk", maxAttempts: 4, baseDelayMs: 1000 },
    );

    for (const contract of chunk) setMark(db, contract.id, latest);
  }

  reconcile(db);
  setMeta(db, LAST_LEDGER, String(latest));
  return total;
}

/** The ledger a contract has been read up to, or where to begin with it. */
function markFor(db: Db, contract: { id: string; from: number }): number {
  const mark = getMeta(db, `ledger:${contract.id}`);
  return mark ? Math.max(Number(mark), 1) : Math.max(contract.from, 1);
}

function setMark(db: Db, contractId: string, ledger: number): void {
  setMeta(db, `ledger:${contractId}`, String(ledger));
}

/** Every event from one group of contracts, following the pages to the end. */
async function readChunk(
  server: rpc.Server,
  contractIds: string[],
  startLedger: number,
  write: (events: Event[]) => void,
): Promise<number> {
  let cursor: string | undefined;
  let total = 0;

  for (let page = 0; page < 50; page += 1) {
    const request = {
      filters: [{ type: "contract" as const, contractIds }],
      limit: 200,
      ...(cursor ? { cursor } : { startLedger }),
    };

    const response = await server.getEvents(request);
    const events = response.events
      .map(toEvent)
      .filter((event): event is Event => event !== null);

    if (events.length > 0) {
      write(events);
      total += events.length;
    }

    if (response.events.length < 200 || !response.cursor) break;
    cursor = response.cursor;
  }

  return total;
}

function toEvent(raw: rpc.Api.EventResponse): Event | null {
  if (!("contractId" in raw) || !raw.contractId) return null;

  const topics: xdr.ScVal[] = raw.topic ?? [];
  const name = topicName(topics);
  if (!name) return null;

  return {
    id: raw.id,
    ledger: raw.ledger,
    ts: Math.floor(new Date(raw.ledgerClosedAt).getTime() / 1000),
    contract: raw.contractId.toString(),
    topic: name,
    topics: topics.map((_, index) => (index === 0 ? name : topicAddress(topics, index))),
    body: decodeBody(raw.value),
    tx: raw.txHash ?? "",
  };
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
