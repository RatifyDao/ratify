import type { Db } from "./db.js";
import { asNumber, asString, type Json } from "./decode.js";

/**
 * One event in, rows out.
 *
 * Every handler is written to be safe to run twice. The indexer can be
 * restarted, or the whole index thrown away and replayed, and the result is
 * the same rows. That is what makes the store disposable, and a disposable
 * store is the only kind whose loss costs nothing but time.
 */

export interface Event {
  id: string;
  ledger: number;
  ts: number;
  contract: string;
  topic: string;
  topics: string[];
  body: { [k: string]: Json };
  tx: string;
}

/** Which community, and which role, an address belongs to. */
export interface Roles {
  byContract: Map<string, { governor: string; role: Role }>;
}

export type Role =
  | "governor"
  | "membership"
  | "weight_rule"
  | "timelock"
  | "treasury"
  | "registry";

/**
 * Reads a field from an event body.
 *
 * Contract events arrive either as a named record or as a positional list,
 * depending on how many fields sit outside the topics. Rather than guess per
 * event, every read gives both a name and a position.
 */
function pick(body: { [k: string]: Json }, name: string, index: number): Json | undefined {
  if (name in body) return body[name];
  const value = body["value"];
  if (Array.isArray(value)) return value[index];
  if (index === 0 && value !== undefined) return value;
  return undefined;
}

export function loadRoles(db: Db): Roles {
  const rows = db
    .prepare(
      "SELECT governor, membership, weight_rule, timelock, treasury, registry FROM communities",
    )
    .all() as Array<Record<Role, string>>;

  const byContract = new Map<string, { governor: string; role: Role }>();
  for (const row of rows) {
    const roles: Role[] = [
      "governor",
      "membership",
      "weight_rule",
      "timelock",
      "treasury",
      "registry",
    ];
    for (const role of roles) {
      if (row[role]) byContract.set(row[role], { governor: row.governor, role });
    }
  }
  return { byContract };
}

/**
 * Every contract the indexer follows, and the earliest ledger its events can
 * have been emitted on.
 *
 * A community's contracts are discovered from the factory, which means they
 * are found some time after their first events were emitted. Following them
 * from the ledger they were deployed on rather than from now is the whole
 * difference between an index that holds a community's history and one that
 * starts from whenever the operator happened to notice it.
 */
export function watchedContracts(
  db: Db,
  factoryId: string,
  windowStart: number,
): Array<{ id: string; from: number }> {
  const communities = db
    .prepare(
      "SELECT governor, membership, weight_rule, timelock, treasury, registry, deployed_at FROM communities",
    )
    .all() as Array<Record<Role, string> & { deployed_at: number }>;

  const watched = new Map<string, number>();
  if (factoryId) watched.set(factoryId, windowStart);

  const roles: Role[] = [
    "governor",
    "membership",
    "weight_rule",
    "timelock",
    "treasury",
    "registry",
  ];
  for (const community of communities) {
    // A community's contracts are constructed in the same transaction the
    // factory records it in, so nothing of theirs predates that ledger.
    const from = Math.max(community.deployed_at, 1);
    for (const role of roles) {
      const id = community[role];
      if (id) watched.set(id, Math.min(watched.get(id) ?? from, from));
    }
  }

  return [...watched.entries()].map(([id, from]) => ({ id, from }));
}

export function handle(db: Db, event: Event, roles: Roles, factoryId: string): void {
  db.prepare(
    `INSERT INTO events (id, ledger, ts, contract, topic, tx, body)
     VALUES (?, ?, ?, ?, ?, ?, ?)
     ON CONFLICT(id) DO NOTHING`,
  ).run(
    event.id,
    event.ledger,
    event.ts,
    event.contract,
    event.topic,
    event.tx,
    JSON.stringify(event.body),
  );

  if (event.contract === factoryId) {
    handleFactory(db, event);
    return;
  }

  const owner = roles.byContract.get(event.contract);
  if (!owner) return;

  switch (owner.role) {
    case "governor":
      handleGovernor(db, event, owner.governor);
      break;
    case "membership":
      handleMembership(db, event);
      break;
    case "timelock":
      handleTimelock(db, event, owner.governor);
      break;
    case "treasury":
      handleTreasury(db, event, owner.governor);
      break;
    case "registry":
      handleRegistry(db, event);
      break;
    default:
      break;
  }
}

// ################## THE REGISTER ##################

function handleFactory(db: Db, event: Event): void {
  if (event.topic !== "community_deployed") return;

  const governor = event.topics[1] ?? "";
  const founder = event.topics[2] ?? "";

  db.prepare(
    `INSERT INTO communities
       (governor, idx, name, membership, weight_rule, timelock, treasury, registry, founder, deployed_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
     ON CONFLICT(governor) DO UPDATE SET
       idx = excluded.idx,
       name = excluded.name,
       membership = excluded.membership,
       weight_rule = excluded.weight_rule,
       timelock = excluded.timelock,
       treasury = excluded.treasury,
       registry = excluded.registry,
       founder = excluded.founder,
       deployed_at = excluded.deployed_at`,
  ).run(
    governor,
    asNumber(pick(event.body, "index", 1)),
    asString(pick(event.body, "name", 0)),
    asString(pick(event.body, "membership", 2)),
    asString(pick(event.body, "weight_rule", 3)),
    asString(pick(event.body, "timelock", 4)),
    asString(pick(event.body, "treasury", 5)),
    asString(pick(event.body, "registry", 6)),
    founder,
    event.ledger,
  );
}

// ################## PROPOSALS AND VOTES ##################

function handleGovernor(db: Db, event: Event, governor: string): void {
  switch (event.topic) {
    case "proposal_created": {
      const id = event.topics[1] ?? "";
      const proposer = event.topics[2] ?? "";
      const targets = pick(event.body, "targets", 0);
      const functions = pick(event.body, "functions", 1);
      const args = pick(event.body, "args", 2);
      db.prepare(
        `INSERT INTO proposals
           (id, governor, proposer, description, actions, snapshot, deadline,
            created_ledger, created_at, created_tx)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           description = excluded.description,
           actions = excluded.actions,
           snapshot = excluded.snapshot,
           deadline = excluded.deadline`,
      ).run(
        id,
        governor,
        proposer,
        asString(pick(event.body, "description", 5)),
        JSON.stringify({ targets, functions, args }),
        asNumber(pick(event.body, "vote_snapshot", 3)),
        asNumber(pick(event.body, "vote_end", 4)),
        event.ledger,
        event.ts,
        event.tx,
      );
      break;
    }

    case "vote_cast": {
      const voter = event.topics[1] ?? "";
      const proposalId = event.topics[2] ?? "";
      db.prepare(
        `INSERT INTO votes (proposal_id, voter, governor, vote_type, weight, reason, ledger, ts, tx)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(proposal_id, voter) DO NOTHING`,
      ).run(
        proposalId,
        voter,
        governor,
        asNumber(pick(event.body, "vote_type", 0)),
        asString(pick(event.body, "weight", 1)),
        asString(pick(event.body, "reason", 2)),
        event.ledger,
        event.ts,
        event.tx,
      );
      break;
    }

    case "proposal_queued":
      db.prepare(
        "UPDATE proposals SET queued_ledger = ?, queued_tx = ?, eta = ? WHERE id = ?",
      ).run(event.ledger, event.tx, asNumber(pick(event.body, "eta", 0)), event.topics[1] ?? "");
      break;

    case "proposal_executed":
      db.prepare(
        "UPDATE proposals SET executed_ledger = ?, executed_tx = ? WHERE id = ?",
      ).run(event.ledger, event.tx, event.topics[1] ?? "");
      break;

    case "proposal_cancelled":
      db.prepare(
        "UPDATE proposals SET cancelled_ledger = ?, cancelled_tx = ? WHERE id = ?",
      ).run(event.ledger, event.tx, event.topics[1] ?? "");
      break;

    case "proposal_stopped_with_reason":
      db.prepare(
        `UPDATE proposals
            SET cancelled_ledger = ?, cancelled_tx = ?, cancel_reason = ?, cancelled_by = ?
          WHERE id = ?`,
      ).run(
        event.ledger,
        event.tx,
        asString(pick(event.body, "reason", 0)),
        event.topics[2] ?? "",
        event.topics[1] ?? "",
      );
      break;

    default:
      break;
  }
}

// ################## MEMBERSHIP AND GRANTS ##################

function handleMembership(db: Db, event: Event): void {
  const membership = event.contract;

  switch (event.topic) {
    case "membership_issued": {
      const account = event.topics[1] ?? "";
      db.prepare(
        `INSERT INTO members (membership, account, tokens, since)
         VALUES (?, ?, 1, ?)
         ON CONFLICT(membership, account) DO UPDATE SET tokens = members.tokens + 1`,
      ).run(membership, account, event.ledger);
      break;
    }

    case "membership_revoked": {
      const account = event.topics[1] ?? "";
      db.prepare(
        `UPDATE members SET tokens = MAX(tokens - 1, 0)
          WHERE membership = ? AND account = ?`,
      ).run(membership, account);
      break;
    }

    case "grant_made":
      db.prepare(
        `INSERT INTO grants (membership, account, delegatee, units, expires_at, granted_at, state, ledger)
         VALUES (?, ?, ?, ?, ?, ?, 'live', ?)
         ON CONFLICT(membership, account) DO UPDATE SET
           delegatee = excluded.delegatee,
           units = excluded.units,
           expires_at = excluded.expires_at,
           granted_at = excluded.granted_at,
           state = 'live',
           ledger = excluded.ledger`,
      ).run(
        membership,
        event.topics[1] ?? "",
        event.topics[2] ?? "",
        asString(pick(event.body, "units", 0)),
        asNumber(pick(event.body, "expires_at", 1)),
        event.ledger,
        event.ledger,
      );
      break;

    case "grant_renewed":
      db.prepare(
        `UPDATE grants SET expires_at = ?, granted_at = ?, state = 'live', ledger = ?
          WHERE membership = ? AND account = ?`,
      ).run(
        asNumber(pick(event.body, "expires_at", 0)),
        event.ledger,
        event.ledger,
        membership,
        event.topics[1] ?? "",
      );
      break;

    case "grant_withdrawn":
      db.prepare(
        `UPDATE grants SET state = 'withdrawn', ledger = ?
          WHERE membership = ? AND account = ?`,
      ).run(event.ledger, membership, event.topics[1] ?? "");
      break;

    case "grant_lapsed":
      db.prepare(
        `UPDATE grants SET state = 'lapsed', ledger = ?
          WHERE membership = ? AND account = ?`,
      ).run(event.ledger, membership, event.topics[1] ?? "");
      break;

    default:
      break;
  }
}

// ################## THE QUEUE ##################

function handleTimelock(db: Db, event: Event, governor: string): void {
  switch (event.topic) {
    case "operation_scheduled": {
      const operationId = event.topics[1] ?? "";
      const target = event.topics[2] ?? "";
      const delay = asNumber(pick(event.body, "delay", 4));
      db.prepare(
        `INSERT INTO queue_items
           (operation_id, governor, timelock, target, fn, scheduled_ledger, ready_at, state, tx)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'waiting', ?)
         ON CONFLICT(operation_id) DO UPDATE SET
           scheduled_ledger = excluded.scheduled_ledger,
           ready_at = excluded.ready_at,
           state = 'waiting'`,
      ).run(
        operationId,
        governor,
        event.contract,
        target,
        asString(pick(event.body, "function", 0)),
        event.ledger,
        event.ledger + delay,
        event.tx,
      );
      break;
    }

    case "operation_executed":
      db.prepare(
        `UPDATE queue_items SET state = 'executed', executed_ledger = ?
          WHERE operation_id = ?`,
      ).run(event.ledger, event.topics[1] ?? "");
      break;

    case "operation_cancelled":
      db.prepare(
        `UPDATE queue_items SET state = 'cancelled', cancelled_ledger = ?
          WHERE operation_id = ?`,
      ).run(event.ledger, event.topics[1] ?? "");
      break;

    case "operation_cancelled_with_reason":
      db.prepare(
        `UPDATE queue_items
            SET state = 'cancelled', cancelled_ledger = ?, cancel_reason = ?, cancelled_by = ?
          WHERE operation_id = ?`,
      ).run(
        event.ledger,
        asString(pick(event.body, "reason", 0)),
        event.topics[2] ?? "",
        event.topics[1] ?? "",
      );
      break;

    default:
      break;
  }
}

// ################## THE MONEY ##################

function handleTreasury(db: Db, event: Event, governor: string): void {
  switch (event.topic) {
    case "payment_made":
      db.prepare(
        `INSERT INTO outflows
           (treasury, idx, governor, asset, recipient, amount, declared_proposal,
            proposal_id, ledger, ts, tx)
         VALUES (?, ?, ?, ?, ?, ?, ?, '', ?, ?, ?)
         ON CONFLICT(treasury, idx) DO NOTHING`,
      ).run(
        event.contract,
        asNumber(pick(event.body, "index", 1)),
        governor,
        event.topics[1] ?? "",
        event.topics[2] ?? "",
        asString(pick(event.body, "amount", 0)),
        event.topics[3] ?? "",
        event.ledger,
        event.ts,
        event.tx,
      );
      break;

    case "deposit_made":
      db.prepare(
        `INSERT INTO deposits (treasury, sender, asset, amount, ledger, ts, tx)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(tx, sender, asset) DO NOTHING`,
      ).run(
        event.contract,
        event.topics[2] ?? "",
        event.topics[1] ?? "",
        asString(pick(event.body, "amount", 0)),
        event.ledger,
        event.ts,
        event.tx,
      );
      break;

    case "policy_set":
      db.prepare(
        `INSERT INTO policies
           (treasury, asset, per_payment_cap, window_cap, window_ledgers, removed, ledger)
         VALUES (?, ?, ?, ?, ?, 0, ?)
         ON CONFLICT(treasury, asset) DO UPDATE SET
           per_payment_cap = excluded.per_payment_cap,
           window_cap = excluded.window_cap,
           window_ledgers = excluded.window_ledgers,
           removed = 0,
           ledger = excluded.ledger`,
      ).run(
        event.contract,
        event.topics[1] ?? "",
        asString(pick(event.body, "per_payment_cap", 0)),
        asString(pick(event.body, "window_cap", 1)),
        asNumber(pick(event.body, "window_ledgers", 2)),
        event.ledger,
      );
      break;

    case "policy_removed":
      db.prepare(
        "UPDATE policies SET removed = 1, ledger = ? WHERE treasury = ? AND asset = ?",
      ).run(event.ledger, event.contract, event.topics[1] ?? "");
      break;

    default:
      break;
  }
}

/**
 * Ties each payment to the proposal that authorised it.
 *
 * The treasury takes a proposal identifier as an argument and has no way to
 * check it, so a proposal could name any identifier it liked. The real link is
 * the transaction: funds can only leave through the timelock, and the governor
 * marks the proposal executed in the same transaction, so a payment and its
 * proposal share a transaction hash and nothing else can.
 *
 * Run after each pass. A payment whose proposal has not been indexed yet is
 * left alone and picked up on a later pass.
 */
export function reconcile(db: Db): void {
  db.prepare(
    `UPDATE outflows
        SET proposal_id = COALESCE(
              (SELECT p.id FROM proposals p WHERE p.executed_tx = outflows.tx),
              proposal_id)
      WHERE proposal_id = ''`,
  ).run();
}

// ################## THE DELEGATE RECORD ##################

function handleRegistry(db: Db, event: Event): void {
  if (event.topic !== "account_settled") return;

  db.prepare(
    `INSERT INTO settlements
       (registry, account, proposal_id, turned_up, contested, with_outcome, ledger)
     VALUES (?, ?, ?, ?, ?, ?, ?)
     ON CONFLICT(registry, account, proposal_id) DO NOTHING`,
  ).run(
    event.contract,
    event.topics[1] ?? "",
    event.topics[2] ?? "",
    pick(event.body, "turned_up", 0) ? 1 : 0,
    pick(event.body, "contested", 1) ? 1 : 0,
    pick(event.body, "with_outcome", 2) ? 1 : 0,
    event.ledger,
  );
}
