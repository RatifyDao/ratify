"use client";

import { useCallback, useEffect, useMemo, useState } from "react";

import { ConnectButton, useWallet } from "@/components/wallet";
import { arg, i128, read, send } from "@/lib/chain";
import { amount } from "@/lib/format";
import { nativeToScVal, xdr } from "@stellar/stellar-sdk";

/**
 * 05 New proposal. Members above the threshold.
 *
 * A structured form, not a text box. What changes, what it costs, who
 * receives, over what period.
 *
 * The form asks the treasury, in real time, whether the payment it is about to
 * describe would be accepted, and refuses to submit one that could never
 * execute. Stolla accepts a plain description and a set of raw contract calls,
 * so a voter approves bytes they cannot read and a proposer can pass something
 * that will fail at the last step.
 */

const ZERO_HASH = new Uint8Array(32);

export function ProposeForm({
  governor,
  treasury,
}: {
  governor: string;
  treasury: string;
}) {
  const { address, sign } = useWallet();

  const [title, setTitle] = useState("");
  const [why, setWhy] = useState("");
  const [asset, setAsset] = useState("");
  const [recipient, setRecipient] = useState("");
  const [figure, setFigure] = useState("");

  const [check, setCheck] = useState<{
    state: "idle" | "checking" | "ok" | "no";
    text: string;
  }>({ state: "idle", text: "" });
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{ ok: boolean; text: string } | null>(null);

  /** The amount in stroops, which is what the contract counts in. */
  const stroops = useMemo(() => {
    if (!figure.trim()) return null;
    const [whole = "0", fraction = ""] = figure.trim().split(".");
    if (!/^\d*$/.test(whole) || !/^\d*$/.test(fraction)) return null;
    const padded = (fraction + "0000000").slice(0, 7);
    try {
      return BigInt(`${whole || "0"}${padded}`);
    } catch {
      return null;
    }
  }, [figure]);

  /**
   * Asks the treasury whether this payment would be accepted.
   *
   * The same question the contract will ask itself at execution, asked before
   * anybody votes. A member should not discover a proposal was impossible
   * after they have already approved it.
   */
  const runCheck = useCallback(async () => {
    if (!asset || !recipient || stroops === null || stroops <= 0n) {
      setCheck({ state: "idle", text: "" });
      return;
    }
    setCheck({ state: "checking", text: "Asking the treasury…" });

    const allowed = await read<boolean>(treasury, "would_allow", [
      arg(asset),
      arg(recipient),
      i128(stroops),
    ]);
    const headroom = await read<bigint>(treasury, "window_headroom", [arg(asset)]);

    if (allowed === true) {
      setCheck({
        state: "ok",
        text: `The treasury would accept this. ${
          headroom === null || headroom === undefined
            ? ""
            : `${amount(String(headroom))} left in the current window.`
        }`,
      });
    } else {
      setCheck({
        state: "no",
        text:
          "The treasury would refuse this. It is over a limit, the asset has no policy, or the destination is not allowed. A proposal that cannot execute is not worth voting on.",
      });
    }
  }, [asset, recipient, stroops, treasury]);

  useEffect(() => {
    const timer = setTimeout(() => void runCheck(), 400);
    return () => clearTimeout(timer);
  }, [runCheck]);

  const description = useMemo(() => {
    const lines = [title.trim()];
    if (why.trim()) lines.push("", why.trim());
    if (asset && recipient && stroops !== null) {
      lines.push(
        "",
        `Pays ${figure} to ${recipient} from the community treasury.`,
      );
    }
    return lines.join("\n");
  }, [title, why, asset, recipient, stroops, figure]);

  const ready =
    address !== null &&
    title.trim().length > 0 &&
    asset.length > 0 &&
    recipient.length > 0 &&
    stroops !== null &&
    stroops > 0n &&
    check.state === "ok";

  const submit = useCallback(async () => {
    if (!address || stroops === null) return;
    setBusy(true);
    setResult(null);
    try {
      const targets = nativeToScVal([arg(treasury)], { type: "address" });
      const call: xdr.ScVal[] = [
        // targets
        xdr.ScVal.scvVec([arg(treasury)]),
        // functions
        xdr.ScVal.scvVec([xdr.ScVal.scvSymbol("pay")]),
        // args, one list per call
        xdr.ScVal.scvVec([
          xdr.ScVal.scvVec([
            arg(asset),
            arg(recipient),
            i128(stroops),
            xdr.ScVal.scvBytes(Buffer.from(ZERO_HASH)),
          ]),
        ]),
        nativeToScVal(description, { type: "string" }),
        arg(address),
      ];
      void targets;

      const { hash } = await send(governor, "propose", call, address, sign);
      setResult({
        ok: true,
        text: `Proposed. Transaction ${hash.slice(0, 12)}… Voting opens after the community's voting delay.`,
      });
    } catch (error) {
      setResult({ ok: false, text: (error as Error).message });
    } finally {
      setBusy(false);
    }
  }, [address, asset, description, governor, recipient, sign, stroops, treasury]);

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
        <h3 style={{ margin: 0 }}>A payment from the treasury</h3>
        <ConnectButton />
      </div>

      <label className="field">
        <span className="label">What this is</span>
        <input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Pay the roofer"
          maxLength={120}
        />
      </label>

      <label className="field">
        <span className="label">Why</span>
        <textarea
          value={why}
          onChange={(e) => setWhy(e.target.value)}
          rows={4}
          placeholder="The reason a member who was not at the meeting can understand."
          maxLength={2000}
        />
      </label>

      <label className="field">
        <span className="label">Asset</span>
        <input
          value={asset}
          onChange={(e) => setAsset(e.target.value.trim())}
          placeholder="C…"
          className="mono"
          spellCheck={false}
        />
      </label>

      <label className="field">
        <span className="label">Who receives</span>
        <input
          value={recipient}
          onChange={(e) => setRecipient(e.target.value.trim())}
          placeholder="G… or C…"
          className="mono"
          spellCheck={false}
        />
      </label>

      <label className="field">
        <span className="label">Amount</span>
        <input
          value={figure}
          onChange={(e) => setFigure(e.target.value)}
          inputMode="decimal"
          placeholder="0.0000000"
          className="fig"
        />
      </label>

      {check.state !== "idle" ? (
        <div
          className={`notice ${
            check.state === "ok"
              ? "notice--authority"
              : check.state === "no"
                ? "notice--outflow"
                : ""
          }`}
          role="status"
        >
          {check.text}
        </div>
      ) : (
        <p className="note">
          Fill in the asset, the recipient and the amount, and the treasury will
          be asked whether it would accept the payment before you propose it.
        </p>
      )}

      <details style={{ margin: "var(--s-4) 0" }}>
        <summary className="label" style={{ cursor: "pointer" }}>
          What members will read
        </summary>
        <pre
          className="mono"
          style={{
            whiteSpace: "pre-wrap",
            background: "var(--ground)",
            padding: "var(--s-3)",
            border: "1px solid var(--rule)",
            borderRadius: "var(--radius)",
            marginTop: "var(--s-2)",
          }}
        >
          {description || "—"}
        </pre>
        <p className="note">
          The description and the calls are hashed together when the proposal is
          made, so the words and the action cannot drift apart afterwards.
        </p>
      </details>

      <button
        type="button"
        className="button"
        disabled={!ready || busy}
        onClick={submit}
      >
        {busy ? "Signing…" : "Propose"}
      </button>

      {!address ? (
        <p className="note" style={{ marginTop: "var(--s-3)" }}>
          Connect a wallet to propose. You need voting power above this
          community&rsquo;s threshold, which means a live grant.
        </p>
      ) : null}

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
  );
}
