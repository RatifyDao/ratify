import Link from "next/link";

import { countdown, shortAddress, shortHash } from "@/lib/format";

/**
 * The small pieces every page is built from.
 *
 * Kept together because each of them encodes a rule from the design system,
 * and a rule that lives in one component is a rule that cannot drift between
 * pages.
 */

/** A figure with its label, set the way a bank statement sets one. */
export function Stat({
  label,
  value,
  note,
  tone,
}: {
  label: string;
  value: string;
  note?: string;
  tone?: "outflow" | "quiet";
}) {
  return (
    <div className="stat">
      <span className="label">{label}</span>
      <div
        className="stat__value"
        style={{
          color:
            tone === "outflow"
              ? "var(--outflow)"
              : tone === "quiet"
                ? "var(--muted)"
                : undefined,
        }}
      >
        {value}
      </div>
      {note ? <div className="stat__note">{note}</div> : null}
    </div>
  );
}

/**
 * A countdown, in ledgers, with the time beside it marked as an estimate.
 *
 * The ledger count is the fact. Ledger time drifts, and a queue timer that
 * shows only "about two days" asks a member to trust an assumption nobody
 * controls.
 */
export function Countdown({ ledgers }: { ledgers: number }) {
  const { ledgers: count, estimate } = countdown(ledgers);
  if (ledgers <= 0) {
    return <span className="chip chip--positive">Ready now</span>;
  }
  return (
    <span>
      <span className="fig">{count}</span> ledgers{" "}
      <span className="note">({estimate}, an estimate)</span>
    </span>
  );
}

/** An address, shortened in the middle, with the whole of it on hover. */
export function Address({ value, keep = 6 }: { value: string | null; keep?: number }) {
  if (!value) return <span className="note">—</span>;
  return (
    <span className="mono" title={value}>
      {shortAddress(value, keep)}
    </span>
  );
}

export function Hash({ value, keep = 8 }: { value: string | null; keep?: number }) {
  if (!value) return <span className="note">—</span>;
  return (
    <span className="mono" title={value}>
      {shortHash(value, keep)}
    </span>
  );
}

/** A link out to the ledger entry a figure came from. */
export function LedgerLink({ tx, label }: { tx: string | null; label?: string }) {
  if (!tx) return <span className="note">—</span>;
  return (
    <a
      className="mono"
      href={`https://stellar.expert/explorer/testnet/tx/${tx}`}
      target="_blank"
      rel="noreferrer"
      title={tx}
    >
      {label ?? shortHash(tx)}
    </a>
  );
}

/**
 * An empty state, told truthfully.
 *
 * A community with no executed proposals says so. Nothing here invents demo
 * data to fill a panel out.
 */
export function Empty({ children }: { children: React.ReactNode }) {
  return <div className="empty">{children}</div>;
}

/**
 * What the interface says when there is no index yet.
 *
 * Not an error. The project has been cloned and the indexer has not been run,
 * which is a real state with a plain answer.
 */
export function NoIndex() {
  return (
    <div className="notice notice--caution">
      <strong>There is no index yet.</strong> RatifyDAO reads governance history
      from an index of contract events rather than from an RPC provider, so
      history outlives what a provider keeps. Deploy a factory, set{" "}
      <span className="mono">RATIFY_FACTORY_ID</span>, and run{" "}
      <span className="mono">npm start</span> in <span className="mono">indexer/</span>.
    </div>
  );
}

/** The per-community navigation. Every screen on the path in one row. */
export function CommunityNav({
  governor,
  current,
}: {
  governor: string;
  current: string;
}) {
  const pages = [
    { href: "", label: "Home" },
    { href: "/proposals", label: "Proposals" },
    { href: "/queue", label: "The queue" },
    { href: "/treasury", label: "Treasury" },
    { href: "/delegates", label: "Delegates" },
    { href: "/membership", label: "Your membership" },
  ];

  return (
    <nav
      aria-label="This community"
      style={{
        display: "flex",
        gap: "var(--s-4)",
        flexWrap: "wrap",
        fontSize: "var(--step--1)",
        borderBottom: "1px solid var(--rule)",
        paddingBottom: "var(--s-2)",
        marginBottom: "var(--s-6)",
      }}
    >
      {pages.map((page) => {
        const active = page.label === current;
        return (
          <Link
            key={page.label}
            href={`/c/${governor}${page.href}`}
            aria-current={active ? "page" : undefined}
            style={{
              textDecoration: "none",
              color: active ? "var(--ink)" : "var(--muted)",
              borderBottom: `2px solid ${active ? "var(--authority)" : "transparent"}`,
              paddingBottom: "var(--s-2)",
            }}
          >
            {page.label}
          </Link>
        );
      })}
    </nav>
  );
}

/** A bar showing how a vote is going, or how far a quorum has to go. */
export function VoteBar({
  forVotes,
  againstVotes,
  abstainVotes,
}: {
  forVotes: bigint;
  againstVotes: bigint;
  abstainVotes: bigint;
}) {
  const total = forVotes + againstVotes + abstainVotes;
  if (total === 0n) {
    return <div className="bar" aria-hidden="true" />;
  }
  const share = (part: bigint) => `${(Number(part) / Number(total)) * 100}%`;
  return (
    <div className="bar">
      <span className="bar__for" style={{ width: share(forVotes) }} />
      <span className="bar__against" style={{ width: share(againstVotes) }} />
      <span className="bar__abstain" style={{ width: share(abstainVotes) }} />
    </div>
  );
}
