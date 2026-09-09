/**
 * Improvement 4 — Health endpoint.
 *
 * A minimal HTTP server on RATIFY_HEALTH_PORT (default 9090) with two routes:
 *
 *   GET /health  — always 200 if the process is alive
 *   GET /ready   — 200 once the indexer has written its first ledger mark;
 *                  503 before then (useful for container readiness probes)
 *
 * The server is intentionally tiny: no framework, no dependencies beyond
 * Node's built-in `http` module. It must not crash the indexer if it fails.
 *
 * Usage:
 *   import { startHealthServer } from "./health.js";
 *   const health = startHealthServer();
 *   health.setReady(true);    // call once the first poll succeeds
 */

import { createServer, type Server } from "node:http";
import { log } from "./logger.js";

export interface HealthServer {
  setReady(ready: boolean): void;
  close(): void;
}

/** Starts the health server and returns a handle for updating readiness. */
export function startHealthServer(port?: number): HealthServer {
  const listenPort = port ?? Number(process.env.RATIFY_HEALTH_PORT ?? 9090);
  let isReady = false;
  let server: Server | null = null;

  try {
    server = createServer((req, res) => {
      if (req.method !== "GET") {
        res.writeHead(405).end();
        return;
      }

      if (req.url === "/health") {
        res.writeHead(200, { "Content-Type": "application/json" }).end(
          JSON.stringify({ status: "ok", ts: Date.now() }),
        );
        return;
      }

      if (req.url === "/ready") {
        const code = isReady ? 200 : 503;
        res.writeHead(code, { "Content-Type": "application/json" }).end(
          JSON.stringify({ ready: isReady, ts: Date.now() }),
        );
        return;
      }

      res.writeHead(404).end();
    });

    server.on("error", (err) => {
      // Port conflict or permission error. Log and continue — the indexer
      // works without the health server.
      log.warn("Health server error", { error: (err as Error).message });
    });

    server.listen(listenPort, () => {
      log.info("Health server listening", { port: listenPort });
    });
  } catch (err) {
    log.warn("Could not start health server", { error: (err as Error).message });
  }

  return {
    setReady(ready: boolean) {
      isReady = ready;
    },
    close() {
      server?.close();
    },
  };
}
