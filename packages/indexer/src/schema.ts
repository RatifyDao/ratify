/**
 * The shape of the index.
 *
 * The ledger is the source of truth and this is a read model, rebuildable by
 * replaying events from the first ledger a community was deployed on. Losing
 * it costs time and nothing else, which is why every table can be dropped and
 * refilled without asking anyone's permission.
 *
 * Amounts are stored as text, not as SQLite integers. Stellar amounts are
 * i128 and a JavaScript number would quietly round the large ones; the
 * interface formats from the string.
 */
export const SCHEMA = `
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

-- Where the indexer has read up to, and anything else it needs to remember
-- between runs.
CREATE TABLE IF NOT EXISTS meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- Every community on the factory's register.
CREATE TABLE IF NOT EXISTS communities (
  governor      TEXT PRIMARY KEY,
  idx           INTEGER NOT NULL,
  name          TEXT NOT NULL DEFAULT '',
  membership    TEXT NOT NULL,
  weight_rule   TEXT NOT NULL,
  timelock      TEXT NOT NULL,
  treasury      TEXT NOT NULL,
  registry      TEXT NOT NULL,
  founder       TEXT NOT NULL DEFAULT '',
  deployed_at   INTEGER NOT NULL DEFAULT 0
);

-- A proposal, and every stage it has reached.
CREATE TABLE IF NOT EXISTS proposals (
  id             TEXT PRIMARY KEY,
  governor       TEXT NOT NULL,
  proposer       TEXT NOT NULL,
  description    TEXT NOT NULL DEFAULT '',
  actions        TEXT NOT NULL DEFAULT '[]',
  snapshot       INTEGER NOT NULL DEFAULT 0,
  deadline       INTEGER NOT NULL DEFAULT 0,
  created_ledger INTEGER NOT NULL DEFAULT 0,
  created_at     INTEGER NOT NULL DEFAULT 0,
  created_tx     TEXT NOT NULL DEFAULT '',
  queued_ledger  INTEGER,
  queued_tx      TEXT,
  eta            INTEGER,
  executed_ledger INTEGER,
  executed_tx    TEXT,
  cancelled_ledger INTEGER,
  cancelled_tx   TEXT,
  cancel_reason  TEXT,
  cancelled_by   TEXT
);
CREATE INDEX IF NOT EXISTS proposals_by_community ON proposals(governor, created_ledger DESC);

-- Every vote cast, with the weight it carried.
CREATE TABLE IF NOT EXISTS votes (
  proposal_id TEXT NOT NULL,
  voter       TEXT NOT NULL,
  governor    TEXT NOT NULL,
  vote_type   INTEGER NOT NULL,
  weight      TEXT NOT NULL,
  reason      TEXT NOT NULL DEFAULT '',
  ledger      INTEGER NOT NULL,
  ts          INTEGER NOT NULL,
  tx          TEXT NOT NULL,
  PRIMARY KEY (proposal_id, voter)
);
CREATE INDEX IF NOT EXISTS votes_by_voter ON votes(voter);

-- Everything sitting in a timelock, and what became of it.
CREATE TABLE IF NOT EXISTS queue_items (
  operation_id TEXT PRIMARY KEY,
  governor     TEXT NOT NULL DEFAULT '',
  timelock     TEXT NOT NULL,
  target       TEXT NOT NULL,
  fn           TEXT NOT NULL DEFAULT '',
  scheduled_ledger INTEGER NOT NULL,
  ready_at     INTEGER NOT NULL,
  state        TEXT NOT NULL,
  executed_ledger INTEGER,
  cancelled_ledger INTEGER,
  cancel_reason TEXT,
  cancelled_by TEXT,
  tx           TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS queue_by_timelock ON queue_items(timelock, ready_at);

-- Money leaving a treasury. The event the whole product exists to produce.
--
-- The declared_proposal column is the identifier the proposal put in its own
-- call, which the treasury has no way to check. The proposal_id column is the
-- one derived from the transaction the payment happened in: funds can only
-- leave through the timelock, and the governor marks the proposal executed in
-- the same transaction, so the link is a fact rather than a claim.
CREATE TABLE IF NOT EXISTS outflows (
  treasury    TEXT NOT NULL,
  idx         INTEGER NOT NULL,
  governor    TEXT NOT NULL DEFAULT '',
  asset       TEXT NOT NULL,
  recipient   TEXT NOT NULL,
  amount      TEXT NOT NULL,
  declared_proposal TEXT NOT NULL DEFAULT '',
  proposal_id TEXT NOT NULL,
  ledger      INTEGER NOT NULL,
  ts          INTEGER NOT NULL,
  tx          TEXT NOT NULL,
  PRIMARY KEY (treasury, idx)
);

-- Money arriving.
CREATE TABLE IF NOT EXISTS deposits (
  treasury TEXT NOT NULL,
  sender   TEXT NOT NULL,
  asset    TEXT NOT NULL,
  amount   TEXT NOT NULL,
  ledger   INTEGER NOT NULL,
  ts       INTEGER NOT NULL,
  tx       TEXT NOT NULL,
  PRIMARY KEY (tx, sender, asset)
);

-- The spending policy in force for each asset, as governance last set it.
CREATE TABLE IF NOT EXISTS policies (
  treasury        TEXT NOT NULL,
  asset           TEXT NOT NULL,
  per_payment_cap TEXT NOT NULL,
  window_cap      TEXT NOT NULL,
  window_ledgers  INTEGER NOT NULL,
  removed         INTEGER NOT NULL DEFAULT 0,
  ledger          INTEGER NOT NULL,
  PRIMARY KEY (treasury, asset)
);

-- A grant of voting power, live or lapsed.
CREATE TABLE IF NOT EXISTS grants (
  membership TEXT NOT NULL,
  account    TEXT NOT NULL,
  delegatee  TEXT NOT NULL,
  units      TEXT NOT NULL,
  expires_at INTEGER NOT NULL,
  granted_at INTEGER NOT NULL DEFAULT 0,
  state      TEXT NOT NULL,
  ledger     INTEGER NOT NULL,
  PRIMARY KEY (membership, account)
);
CREATE INDEX IF NOT EXISTS grants_by_delegatee ON grants(membership, delegatee);

-- Membership itself.
CREATE TABLE IF NOT EXISTS members (
  membership TEXT NOT NULL,
  account    TEXT NOT NULL,
  tokens     INTEGER NOT NULL DEFAULT 0,
  since      INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (membership, account)
);

-- A delegate's public record, as the registry settles it.
CREATE TABLE IF NOT EXISTS settlements (
  registry    TEXT NOT NULL,
  account     TEXT NOT NULL,
  proposal_id TEXT NOT NULL,
  turned_up   INTEGER NOT NULL,
  contested   INTEGER NOT NULL,
  with_outcome INTEGER NOT NULL,
  ledger      INTEGER NOT NULL,
  PRIMARY KEY (registry, account, proposal_id)
);

-- Every event the indexer has seen, kept raw. The operations page reads this,
-- and it is what makes a rebuild verifiable rather than merely possible.
CREATE TABLE IF NOT EXISTS events (
  id       TEXT PRIMARY KEY,
  ledger   INTEGER NOT NULL,
  ts       INTEGER NOT NULL,
  contract TEXT NOT NULL,
  topic    TEXT NOT NULL,
  tx       TEXT NOT NULL,
  body     TEXT NOT NULL,
  handled  INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX IF NOT EXISTS events_by_ledger ON events(ledger DESC);
CREATE INDEX IF NOT EXISTS events_by_contract ON events(contract, ledger DESC);
`;
