#!/usr/bin/env node
/**
 * Puts a real proposal through a real community on testnet.
 *
 * Propose, vote, queue, wait out the delay, execute, and watch a treasury
 * balance change. The same path the tests prove in a simulated environment,
 * run against a live network so the interface has something true to show.
 *
 * The Stellar CLI cannot express a proposal's arguments, because they are a
 * list of lists of contract values and the CLI has no way to know their types.
 * That is the whole reason this script exists rather than a shell one.
 *
 *   node scripts/seed-testnet.mjs policy    set a spending policy by vote
 *   node scripts/seed-testnet.mjs pay       pay someone by vote
 *
 * Addresses come from ratify.testnet.json in the repository root, which
 * `stellar contract deploy` and the factory produced.
 */

import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { keccak256 } from "js-sha3";

import {
  Address,
  BASE_FEE,
  Contract,
  Keypair,
  TransactionBuilder,
  nativeToScVal,
  rpc,
  scValToNative,
  xdr,
} from "@stellar/stellar-sdk";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const RPC = process.env.RATIFY_RPC_URL ?? "https://soroban-testnet.stellar.org";
const PASSPHRASE =
  process.env.RATIFY_NETWORK_PASSPHRASE ?? "Test SDF Network ; September 2015";

const deployment = JSON.parse(
  readFileSync(resolve(root, "ratify.testnet.json"), "utf8"),
);
const server = new rpc.Server(RPC);

/** The secret for a CLI identity. Never printed, only used to sign. */
function keypair(alias) {
  const secret = execFileSync("stellar", ["keys", "show", alias], {
    encoding: "utf8",
  }).trim();
  return Keypair.fromSecret(secret);
}

const ZERO = xdr.ScVal.scvBytes(Buffer.alloc(32));
const addr = (value) => new Address(value).toScVal();
const sym = (value) => xdr.ScVal.scvSymbol(value);
const u32 = (value) => nativeToScVal(value, { type: "u32" });
const i128 = (value) => nativeToScVal(BigInt(value), { type: "i128" });
const str = (value) => nativeToScVal(value, { type: "string" });
const vec = (items) => xdr.ScVal.scvVec(items);

async function call(contractId, method, args, signer, { quiet = false } = {}) {
  const account = await server.getAccount(signer.publicKey());
  const built = new TransactionBuilder(account, {
    fee: (Number(BASE_FEE) * 100).toString(),
    networkPassphrase: PASSPHRASE,
  })
    .addOperation(new Contract(contractId).call(method, ...args))
    .setTimeout(120)
    .build();

  const prepared = await server.prepareTransaction(built);
  prepared.sign(signer);

  const sent = await server.sendTransaction(prepared);
  if (sent.status === "ERROR") {
    throw new Error(`${method} was refused: ${JSON.stringify(sent.errorResult)}`);
  }

  for (let i = 0; i < 40; i += 1) {
    const result = await server.getTransaction(sent.hash);
    if (result.status === "SUCCESS") {
      if (!quiet) console.log(`  ${method} ✓ ${sent.hash.slice(0, 12)}…`);
      return result.returnValue ? scValToNative(result.returnValue) : null;
    }
    if (result.status === "FAILED") {
      throw new Error(`${method} failed on chain: ${sent.hash}`);
    }
    await sleep(1000);
  }
  throw new Error(`${method} was not included in time: ${sent.hash}`);
}

/**
 * A stage that may already have happened.
 *
 * The script is meant to be safe to re-run: a network hiccup between voting
 * and queuing should not mean starting a governance cycle again. Each stage
 * names the errors that mean "already done" and steps over them.
 */
async function stage(contractId, method, args, signer, alreadyDone) {
  try {
    return await call(contractId, method, args, signer);
  } catch (error) {
    const text = String(error?.message ?? error);
    for (const [code, note] of Object.entries(alreadyDone)) {
      if (text.includes(`#${code}`)) {
        console.log(`  ${method} — ${note}`);
        return null;
      }
    }
    throw error;
  }
}

async function read(contractId, method, args = []) {
  const account = await server.getAccount(deployment.founder);
  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: PASSPHRASE,
  })
    .addOperation(new Contract(contractId).call(method, ...args))
    .setTimeout(30)
    .build();
  const simulated = await server.simulateTransaction(tx);
  if (!rpc.Api.isSimulationSuccess(simulated) || !simulated.result) return null;
  try {
    return scValToNative(simulated.result.retval);
  } catch (error) {
    throw new Error(`reading ${method}: ${error.message}`);
  }
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function ledger() {
  return (await server.getLatestLedger()).sequence;
}

async function waitFor(target, what) {
  let now = await ledger();
  if (now >= target) return;
  console.log(`  waiting for ledger ${target} (${what}), now ${now}`);
  while (now < target) {
    await sleep(4000);
    now = await ledger();
    process.stdout.write(`\r  ledger ${now}   `);
  }
  process.stdout.write("\n");
}

/**
 * The whole path, once.
 *
 * Every stage is a separate transaction anyone could send, which is the
 * point: nothing here needs a privileged key, and after the delay the
 * execution could be sent by a stranger.
 */
async function govern(description, targets, functions, args) {
  const founder = keypair("ratify-founder");
  const member = keypair("ratify-member");
  const { governor } = deployment;

  console.log(`\n${description}`);

  const proposalArgs = [
    vec(targets.map(addr)),
    vec(functions.map(sym)),
    vec(args.map((one) => vec(one))),
    str(description),
    addr(deployment.founder),
  ];

  const hash = xdr.ScVal.scvBytes(keccakOfString(description));

  await stage(governor, "propose", proposalArgs, founder, {
    5001: "already proposed, carrying on",
  });

  const id = await read(governor, "get_proposal_id", [
    vec(targets.map(addr)),
    vec(functions.map(sym)),
    vec(args.map((one) => vec(one))),
    hash,
  ]);
  if (!id) throw new Error("Could not work out the proposal identifier.");
  const proposalId = xdr.ScVal.scvBytes(Buffer.from(id));
  console.log(`  proposal ${Buffer.from(id).toString("hex").slice(0, 16)}…`);

  const snapshot = await read(governor, "proposal_snapshot", [proposalId]);
  const deadline = await read(governor, "proposal_deadline", [proposalId]);

  await waitFor(Number(snapshot) + 1, "voting to open");

  await stage(
    governor,
    "cast_vote",
    [proposalId, u32(1), str("The roof will not wait."), addr(deployment.founder)],
    founder,
    { 5016: "the founder has already voted" },
  );
  await stage(
    governor,
    "cast_vote",
    [proposalId, u32(1), str(""), addr(deployment.member)],
    member,
    { 5016: "the member has already voted" },
  );

  await waitFor(Number(deadline) + 1, "voting to close");

  await stage(
    governor,
    "queue",
    [
      vec(targets.map(addr)),
      vec(functions.map(sym)),
      vec(args.map((one) => vec(one))),
      hash,
      u32(0),
      addr(deployment.founder),
    ],
    founder,
    { 5006: "already queued, carrying on" },
  );

  const eta = Number(await ledger()) + Number(deployment.timelock_delay ?? 12);
  await waitFor(eta, "the timelock delay");

  await stage(
    governor,
    "execute",
    [
      vec(targets.map(addr)),
      vec(functions.map(sym)),
      vec(args.map((one) => vec(one))),
      hash,
      addr(deployment.founder),
    ],
    founder,
    { 5008: "already executed" },
  );

  console.log("  executed.");
}

/**
 * The governor hashes a description with keccak256, so the identifier this
 * script computes has to match the contract's byte for byte. Get this wrong
 * and every later stage refers to a proposal that does not exist, which is
 * exactly the drift the contract's hashing is there to prevent.
 */
function keccakOfString(text) {
  return Buffer.from(keccak256.arrayBuffer(Buffer.from(text, "utf8")));
}

/**
 * Settles both members against every closed proposal.
 *
 * Anyone may do this, for anyone, and the result is fixed by chain state. It
 * is what turns a missed vote into part of a delegate's record, and it is why
 * a delegate cannot avoid one.
 */
async function settle() {
  const { registry } = deployment;
  const founder = keypair("ratify-founder");

  // The proposals to settle come from the index, which is the only place that
  // knows every one a community has ever had.
  const { default: Database } = await import("better-sqlite3");
  const db = new Database(resolve(root, "data", "ratify.db"), { readonly: true });
  const proposals = db
    .prepare("SELECT id FROM proposals WHERE governor = ? ORDER BY created_ledger")
    .all(deployment.governor)
    .map((row) => row.id);
  db.close();

  if (proposals.length === 0) {
    console.log("The index holds no proposals. Run the indexer first.");
    return;
  }

  for (const id of proposals) {
    for (const account of [deployment.founder, deployment.member]) {
      await stage(
        registry,
        "settle",
        [addr(account), xdr.ScVal.scvBytes(Buffer.from(id, "hex"))],
        founder,
        { 6: "already settled", 5: "voting is still open", 7: "was not eligible" },
      );
    }
  }
}

async function main() {
  const what = process.argv[2] ?? "policy";
  // A proposal is identified by its calls and its description together, so a
  // second proposal with the same words is the same proposal. The tag is how
  // to make a genuinely new one.
  const tag = process.argv[3] ? ` (${process.argv[3]})` : "";
  const { treasury, asset } = deployment;

  if (what === "policy") {
    await govern(
      "Set a spending policy on XLM: at most 10 XLM per payment and 30 XLM in a rolling window of about a day." + tag,
      [treasury],
      ["set_policy"],
      [[addr(asset), i128(100_000_000n), i128(300_000_000n), u32(17_280)]],
    );
  } else if (what === "pay") {
    await govern(
      "Pay 4 XLM to the roofer for repairs to the hall." + tag,
      [treasury],
      ["pay"],
      [[addr(asset), addr(deployment.member), i128(40_000_000n), ZERO]],
    );
  } else if (what === "settle") {
    await settle();
    return;
  } else {
    console.error("Say policy, pay or settle.");
    process.exitCode = 1;
    return;
  }

  console.log(
    `\nTreasury balance: ${await read(treasury, "balance", [addr(asset)])}`,
  );
}

main().catch((error) => {
  console.error(error.stack ?? error);
  process.exitCode = 1;
});
