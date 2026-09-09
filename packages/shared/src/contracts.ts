/**
 * Improvement 8 — Contract method name constants.
 *
 * Every method name used in a transaction call is defined here as a typed
 * constant. Passing a raw string like "cast_vote" directly to `contract.call()`
 * means a typo compiles cleanly and fails only at runtime on-chain. Using
 * these constants means the failure is a TypeScript error at build time.
 *
 * Usage:
 *   import { GOVERNOR, MEMBERSHIP } from "@ratify-dao/shared/contracts";
 *
 *   contract.call(GOVERNOR.CAST_VOTE, ...args)
 *   contract.call(MEMBERSHIP.DELEGATE_FOR, ...args)
 */

/** Methods on the ratify-governor contract. */
export const GOVERNOR = {
  PROPOSE:        "propose",
  CAST_VOTE:      "cast_vote",
  QUEUE:          "queue",
  EXECUTE:        "execute",
  CANCEL:         "cancel",
  STOP:           "stop",
  PROPOSAL_STATE: "proposal_state",
  GET_PROPOSAL_ID:"get_proposal_id",
  QUORUM_PROGRESS:"quorum_progress",
  TIMELOCK:       "timelock",
  QUORUM_BPS:     "quorum_bps",
} as const;

/** Methods on the ratify-membership contract. */
export const MEMBERSHIP = {
  DELEGATE_FOR:       "delegate_for",
  RENEW:              "renew",
  WITHDRAW:           "withdraw",
  LAPSE:              "lapse",
  ISSUE:              "issue",
  REVOKE:             "revoke",
  TRANSFER_ISSUANCE:  "transfer_issuance",
  BALANCE:            "balance",
  VOTES:              "votes",
  VOTES_AT:           "votes_at",
  GRANT:              "grant",
  IS_LIVE:            "is_live",
  LEDGERS_UNTIL_LAPSE:"ledgers_until_lapse",
  LIVE_TOTAL:         "live_total",
  MEMBER_COUNT:       "member_count",
  ISSUER:             "issuer",
} as const;

/** Methods on the ratify-timelock contract. */
export const TIMELOCK = {
  SCHEDULE:              "schedule",
  EXECUTE:               "execute",
  CANCEL:                "cancel",
  CANCEL_WITH_REASON:    "cancel_with_reason",
  EXECUTE_DELAY_UPDATE:  "execute_delay_update",
  EXECUTE_GUARDIAN_CHANGE:"execute_guardian_change",
  GET_MIN_DELAY:         "get_min_delay",
  LEDGERS_REMAINING:     "ledgers_remaining",
  GUARDIAN:              "guardian",
  GOVERNOR:              "governor",
} as const;

/** Methods on the ratify-treasury contract. */
export const TREASURY = {
  PAY:                       "pay",
  DEPOSIT:                   "deposit",
  SET_POLICY:                "set_policy",
  REMOVE_POLICY:             "remove_policy",
  SET_DESTINATION:           "set_destination",
  SET_DESTINATION_RESTRICTION:"set_destination_restriction",
  BALANCE:                   "balance",
  POLICY:                    "policy",
  WOULD_ALLOW:               "would_allow",
  WINDOW_HEADROOM:           "window_headroom",
  OUTFLOW:                   "outflow",
  OUTFLOW_COUNT:             "outflow_count",
  TIMELOCK:                  "timelock",
} as const;

/** Methods on the ratify-delegate-registry contract. */
export const REGISTRY = {
  OPEN:                       "open",
  RECORD_VOTE:                "record_vote",
  SETTLE:                     "settle",
  RECORD:                     "record",
  PARTICIPATION_BPS:          "participation_bps",
  CONTESTED_PARTICIPATION_BPS:"contested_participation_bps",
  PROPOSAL:                   "proposal",
  BALLOT:                     "ballot",
  IS_SETTLED:                 "is_settled",
  GOVERNOR:                   "governor",
  WEIGHT_RULE:                "weight_rule",
} as const;

/** Methods on the ratify-factory contract. */
export const FACTORY = {
  DEPLOY:                "deploy",
  COMMUNITY:             "community",
  COMMUNITIES:           "communities",
  COMMUNITY_COUNT:       "community_count",
  COMMUNITY_BY_GOVERNOR: "community_by_governor",
  ADDRESSES_FOR:         "addresses_for",
  WASMS:                 "wasms",
} as const;

/** Methods on the ratify-weight-rule contract. */
export const WEIGHT_RULE = {
  WEIGHT_AT:       "weight_at",
  TOTAL_WEIGHT_AT: "total_weight_at",
  MODEL:           "model",
  GET_VOTES:       "get_votes",
  GET_VOTES_AT_CHECKPOINT: "get_votes_at_checkpoint",
} as const;
