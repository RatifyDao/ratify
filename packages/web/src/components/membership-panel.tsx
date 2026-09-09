"use client";

import { useCallback, useEffect, useState } from "react";

import { ConnectButton, useWallet } from "@/components/wallet";
import { arg, read, send, u32 } from "@/lib/chain";
import { count, countdown, duration } from "@/lib/format";

/**
 * 09 Your membership, the half that needs a signature.
 *
 * Read from the chain rather than from the index, because this is the one
 * thing a member cannot afford to see a stale version of: whether their own
 * power is still live, and exactly when it lapses.
 *
 * This page is why lapsing does not become an annoyance. One action to renew,
 * extend or withdraw, with the date printed rather than hidden.
 */

interface Grant {
  delegatee: string;
  units: string | bigint;
  expires_at: number;
  granted_at: number;
}

const TERMS = [
  { label: "3 months", ledgers: 90 * 17_280 },
  { label: "6 months", ledgers: 182 * 17_280 },
  { label: "1 year", ledgers: 365 * 17_280 },
];

export function MembershipPanel({
  membership,
  ledger,
}: {
  membership: string;
  ledger: number;
}) {
  const { address, sign } = useWallet();

  const [tokens, setTokens] = useState<number | null>(null);
  const [power, setPower] = useState<string | null>(null);
  const [grant, setGrant] = useState<Grant | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [result, setResult] = useState<{ ok: boolean; text: string } | null>(null);

  const [delegatee, setDelegatee] = useState("");
  const [term, setTerm] = useState(TERMS[1]!.ledgers);

  const refresh = useCallback(async () => {
    if (!address) return;
    setLoading(true);
    try {
      const [held, votes, current] = await Promise.all([
        read<number>(membership, "balance", [arg(address)]),
        read<bigint>(membership, "votes", [arg(address)]),
        read<Grant | null>(membership, "grant", [arg(address)]),
      ]);
      setTokens(held ?? 0);
      setPower(votes === null || votes === undefined ? "0" : String(votes));
      setGrant(current ?? null);
    } finally {
      setLoading(false);
    }
  }, [address, membership]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const act = useCallback(
    async (label: string, method: string, args: Parameters<typeof send>[2]) => {
      if (!address) return;
      setBusy(label);
      setResult(null);
      try {
        const { hash } = await send(membership, method, args, address, sign);
        setResult({ ok: true, text: `Done. Transaction ${hash.slice(0, 12)}…` });
        await refresh();
      } catch (error) {
        setResult({ ok: false, text: (error as Error).message });
      } finally {
        setBusy(null);
      }
    },
    [address, membership, sign, refresh],
  );

  if (!address) {
    return (
      <div className="panel">
        <h3>Your membership</h3>
        <p className="note">
          Connect a wallet to see the tokens you hold, the power that answers to
          you, and when your grant lapses.
        </p>
        <ConnectButton />
      </div>
    );
  }

  const live = grant !== null && grant.expires_at > ledger;
  const remaining = grant ? grant.expires_at - ledger : 0;
  const clock = countdown(remaining);

  return (
    <div className="panel">
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "baseline",
          gap: "var(--s-4)",
          flexWrap: "wrap",
          marginBottom: "var(--s-5)",
        }}
      >
        <h3 style={{ margin: 0 }}>Your membership</h3>
        <ConnectButton />
      </div>

      {loading ? <p className="note">Reading the chain…</p> : null}

      <div className="grid" style={{ marginBottom: "var(--s-5)" }}>
        <div className="stat">
          <span className="label">Tokens held</span>
          <div className="stat__value">{count(tokens ?? 0)}</div>
        </div>
        <div className="stat">
          <span className="label">Power answering to you</span>
          <div className="stat__value">{count(power ?? "0")}</div>
          <div className="stat__note">Yours, plus anything granted to you.</div>
        </div>
        <div className="stat">
          <span className="label">Your grant</span>
          <div
            className="stat__value"
            style={{ color: live ? undefined : "var(--muted)" }}
          >
            {grant === null ? "none" : live ? "live" : "lapsed"}
          </div>
          <div className="stat__note">
            {grant === null
              ? "Your tokens count for nothing until you grant their power."
              : live
                ? `Lapses in ${clock.ledgers} ledgers (${clock.estimate}, an estimate)`
                : "It has expired. Anyone may sweep it."}
          </div>
        </div>
      </div>

      {grant !== null ? (
        <div className={`notice ${live ? "" : "notice--caution"}`}>
          {live ? (
            <>
              Your power answers to{" "}
              <span className="mono" title={grant.delegatee}>
                {grant.delegatee.slice(0, 8)}…{grant.delegatee.slice(-8)}
              </span>{" "}
              until ledger <span className="fig">{count(grant.expires_at)}</span>.
            </>
          ) : (
            <>
              <strong>Your grant has lapsed.</strong> It stopped counting towards
              quorum at ledger <span className="fig">{count(grant.expires_at)}</span>.
              Grant it again to take part.
            </>
          )}
        </div>
      ) : null}

      {tokens === 0 ? (
        <div className="notice">
          You hold no membership tokens in this community, so there is nothing to
          grant yet.
        </div>
      ) : null}

      <div style={{ marginTop: "var(--s-5)" }}>
        <span className="label">Grant your power</span>
        <p className="note">
          Leave the address empty to hold your own power. Either way the grant
          carries a term and lapses unless you renew it, which is what keeps
          quorum measured against members who are actually here.
        </p>

        <label className="field">
          <span className="label">Delegate to</span>
          <input
            value={delegatee}
            onChange={(e) => setDelegatee(e.target.value.trim())}
            placeholder={address}
            className="mono"
            spellCheck={false}
          />
        </label>

        <label className="field">
          <span className="label">Term</span>
          <select value={term} onChange={(e) => setTerm(Number(e.target.value))}>
            {TERMS.map((option) => (
              <option key={option.ledgers} value={option.ledgers}>
                {option.label} ({count(option.ledgers)} ledgers)
              </option>
            ))}
          </select>
        </label>

        <div style={{ display: "flex", gap: "var(--s-3)", flexWrap: "wrap" }}>
          <button
            type="button"
            className="button"
            disabled={busy !== null || tokens === 0}
            onClick={() =>
              act("grant", "delegate_for", [
                arg(address),
                arg(delegatee || address),
                u32(term),
              ])
            }
          >
            {busy === "grant" ? "Signing…" : grant ? "Grant again" : "Grant for this term"}
          </button>

          <button
            type="button"
            className="button button--quiet"
            disabled={busy !== null || !live}
            onClick={() => act("renew", "renew", [arg(address), u32(term)])}
            title={live ? undefined : "A lapsed grant is made again, not renewed."}
          >
            {busy === "renew" ? "Signing…" : `Renew for ${duration(term)}`}
          </button>

          <button
            type="button"
            className="button button--quiet"
            disabled={busy !== null || grant === null}
            onClick={() => act("withdraw", "withdraw", [arg(address)])}
          >
            {busy === "withdraw" ? "Signing…" : "Withdraw"}
          </button>
        </div>

        {result ? (
          <div
            className={`notice ${result.ok ? "notice--authority" : "notice--outflow"}`}
            style={{ marginTop: "var(--s-4)" }}
            role="status"
          >
            {result.text}
          </div>
        ) : null}
      </div>
    </div>
  );
}
