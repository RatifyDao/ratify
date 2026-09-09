"use client";

import {
  Account,
  Address,
  BASE_FEE,
  Contract,
  Networks,
  TransactionBuilder,
  nativeToScVal,
  rpc,
  scValToNative,
  xdr,
} from "@stellar/stellar-sdk";

/**
 * The only place the interface talks to the chain directly.
 *
 * Public pages read the index, never this. What is left is the connected
 * member's own state and the actions they take, both of which have to be
 * current to the ledger rather than to the last time an indexer ran: your own
 * voting power, whether your grant is still live, and the transactions you
 * sign.
 */

// Improvement 7 — address validation before any contract call.
import { assertAddress } from "../../shared/src/addresses.js";

export const NETWORK = {
  rpcUrl: process.env.NEXT_PUBLIC_RATIFY_RPC_URL ?? "https://soroban-testnet.stellar.org",
  passphrase: process.env.NEXT_PUBLIC_RATIFY_NETWORK_PASSPHRASE ?? Networks.TESTNET,
};

function server(): rpc.Server {
  return new rpc.Server(NETWORK.rpcUrl, { allowHttp: NETWORK.rpcUrl.startsWith("http://") });
}

/** Turns everyday values into contract arguments. */
export function arg(value: unknown): xdr.ScVal {
  if (typeof value === "string" && (value.startsWith("C") || value.startsWith("G"))) {
    return new Address(value).toScVal();
  }
  return nativeToScVal(value);
}

export function u32(value: number): xdr.ScVal {
  return nativeToScVal(value, { type: "u32" });
}

export function i128(value: bigint | string): xdr.ScVal {
  return nativeToScVal(BigInt(value), { type: "i128" });
}

/**
 * Reads a contract without spending anything.
 *
 * A simulation, so it costs nothing and changes nothing. Used for the
 * connected member's own state, and for the preview on the proposal form that
 * asks the treasury whether a payment would be accepted.
 */
export async function read<T = unknown>(
  contractId: string,
  method: string,
  args: xdr.ScVal[] = [],
  source = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
): Promise<T | null> {
  assertAddress(contractId, "contractId"); // Improvement 7
  try {
    const rpcServer = server();
    // A read costs nothing and changes nothing, so the source account only
    // has to exist as far as the simulation is concerned.
    const account = await rpcServer
      .getAccount(source)
      .catch(() => new Account(source, "0"));
    const contract = new Contract(contractId);
    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: NETWORK.passphrase,
    })
      .addOperation(contract.call(method, ...args))
      .setTimeout(30)
      .build();

    const simulated = await rpcServer.simulateTransaction(tx);
    if (!rpc.Api.isSimulationSuccess(simulated) || !simulated.result) return null;
    return scValToNative(simulated.result.retval) as T;
  } catch {
    return null;
  }
}

/**
 * Signs and submits a call.
 *
 * The wallet does the signing; nothing here ever sees a key. The returned
 * hash is what the interface links to, so a member can check what they just
 * did against the ledger rather than against a message from this page.
 */
export async function send(
  contractId: string,
  method: string,
  args: xdr.ScVal[],
  publicKey: string,
  sign: (xdr: string) => Promise<string>,
): Promise<{ hash: string }> {
  assertAddress(contractId, "contractId"); // Improvement 7
  assertAddress(publicKey,  "publicKey");  // Improvement 7
  const rpcServer = server();
  const account = await rpcServer.getAccount(publicKey);
  const contract = new Contract(contractId);

  const built = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK.passphrase,
  })
    .addOperation(contract.call(method, ...args))
    .setTimeout(60)
    .build();

  const prepared = await rpcServer.prepareTransaction(built);
  const signed = await sign(prepared.toXDR());
  const tx = TransactionBuilder.fromXDR(signed, NETWORK.passphrase);

  const sent = await rpcServer.sendTransaction(tx);
  if (sent.status === "ERROR") {
    throw new Error("The network refused the transaction.");
  }

  // Wait for it to be included, so the interface reports what happened rather
  // than what was attempted.
  for (let i = 0; i < 30; i += 1) {
    const result = await rpcServer.getTransaction(sent.hash);
    if (result.status === rpc.Api.GetTransactionStatus.SUCCESS) return { hash: sent.hash };
    if (result.status === rpc.Api.GetTransactionStatus.FAILED) {
      throw new Error("The transaction was included and failed.");
    }
    await new Promise((r) => setTimeout(r, 1000));
  }

  throw new Error("The transaction has not been included yet. Check the ledger.");
}
