/**
 * Deployed contract addresses, keyed by network.
 *
 * This is the single place in the codebase that knows where the contracts
 * live. The indexer reads it to bootstrap, the web app reads it to pre-fill
 * contract IDs, and neither has to parse a JSON file at runtime.
 *
 * When a new network deployment happens, add an entry here and nowhere else.
 */

export type Network = "testnet" | "mainnet";

export interface DeployedAddresses {
  factory: string;
  delegateRegistry: string;
}

export const DEPLOYED: Record<Network, DeployedAddresses> = {
  testnet: {
    factory:          "CBTXUGABEUSSOZCYQ5LEMLK2FXGK5EIEOO6S23R4MHPEUVZ5ETOFCJO2",
    delegateRegistry: "CBW7F2UDNJW3FZZDKCQXXYYMO3SJWKMDHEKPA3CLUECBFJNNF3TGC5AA",
  },
  mainnet: {
    factory:          "",
    delegateRegistry: "",
  },
};

/** Returns addresses for the active network, defaulting to testnet. */
export function deployedAddresses(network?: string): DeployedAddresses {
  if (network === "mainnet") return DEPLOYED.mainnet;
  return DEPLOYED.testnet;
}

/** Validates that a string looks like a Stellar contract address (C…, 56 chars). */
export function isContractAddress(value: string): boolean {
  return /^C[A-Z2-7]{55}$/.test(value);
}

/** Validates that a string looks like a Stellar account address (G…, 56 chars). */
export function isAccountAddress(value: string): boolean {
  return /^G[A-Z2-7]{55}$/.test(value);
}

/** Validates that a value is any valid Stellar strkey (contract or account). */
export function isStellarAddress(value: string): boolean {
  return isContractAddress(value) || isAccountAddress(value);
}

/** Throws if a value is not a valid Stellar address. */
export function assertAddress(value: string, label = "address"): void {
  if (!isStellarAddress(value)) {
    throw new Error(`Invalid ${label}: "${value}" is not a valid Stellar address.`);
  }
}
