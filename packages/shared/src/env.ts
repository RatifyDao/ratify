/**
 * Environment variable validation.
 *
 * Called once at startup in both the indexer and the web app. A missing
 * required variable fails loudly with a clear message rather than silently
 * producing wrong behaviour at runtime.
 *
 * Optional variables are documented here even when they are not required, so
 * there is one place to look for what the project reads from the environment.
 */

export interface EnvSpec {
  /** The variable name. */
  name: string;
  /** Whether the process should refuse to start without it. */
  required: boolean;
  /** Shown in the error message when the variable is missing. */
  description: string;
}

/** Every environment variable RatifyDAO reads, with its requirement level. */
export const ENV_SPEC: EnvSpec[] = [
  {
    name: "RATIFY_RPC_URL",
    required: false,
    description: "Soroban RPC endpoint (defaults to testnet)",
  },
  {
    name: "RATIFY_NETWORK_PASSPHRASE",
    required: false,
    description: "Stellar network passphrase (defaults to testnet)",
  },
  {
    name: "RATIFY_FACTORY_ID",
    required: true,
    description: "The factory contract address whose register the indexer follows",
  },
  {
    name: "RATIFY_DB_PATH",
    required: false,
    description: "Path to the SQLite index file (defaults to ./data/ratify.db)",
  },
  {
    name: "RATIFY_POLL_MS",
    required: false,
    description: "How often the indexer polls for new events in milliseconds (default 5000)",
  },
  {
    name: "NEXT_PUBLIC_RATIFY_RPC_URL",
    required: false,
    description: "RPC URL exposed to the browser (Next.js public env var)",
  },
  {
    name: "NEXT_PUBLIC_RATIFY_NETWORK_PASSPHRASE",
    required: false,
    description: "Network passphrase exposed to the browser (Next.js public env var)",
  },
];

/**
 * Validates the environment against the spec.
 *
 * Returns a list of error messages. An empty list means the environment is
 * valid. Callers decide whether to throw, log or exit.
 */
export function validateEnv(env: NodeJS.ProcessEnv = process.env): string[] {
  const errors: string[] = [];
  for (const spec of ENV_SPEC) {
    if (spec.required && !env[spec.name]) {
      errors.push(`Missing required env var ${spec.name}: ${spec.description}`);
    }
  }
  return errors;
}

/**
 * Throws if any required environment variables are missing.
 * Call this once at startup, before doing anything else.
 */
export function assertEnv(env: NodeJS.ProcessEnv = process.env): void {
  const errors = validateEnv(env);
  if (errors.length > 0) {
    throw new Error(
      `RatifyDAO cannot start. Fix the following:\n${errors.map((e) => `  • ${e}`).join("\n")}`,
    );
  }
}
