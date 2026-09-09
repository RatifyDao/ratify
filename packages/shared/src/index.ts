/**
 * @ratify-dao/shared
 *
 * Single source of truth for types, addresses, constants, and environment
 * validation shared between the indexer and the web app.
 *
 * Import from sub-paths for tree-shaking in the web bundle:
 *
 *   import type { Proposal } from "@ratify-dao/shared/types";
 *   import { assertAddress } from "@ratify-dao/shared/addresses";
 *   import { formatAmount } from "@ratify-dao/shared/constants";
 *   import { assertEnv } from "@ratify-dao/shared/env";
 *
 * Or import everything for the indexer (Node.js, bundle size irrelevant):
 *
 *   import * as Shared from "@ratify-dao/shared";
 */

export * from "./types.js";
export * from "./addresses.js";
export * from "./constants.js";
export * from "./env.js";
export * from "./contracts.js";
export * from "./proposal-state.js";
