import { notFound } from "next/navigation";

import {
  Address,
  CommunityNav,
  Countdown,
  Empty,
  Hash,
  LedgerLink,
} from "@/components/pieces";
import { count } from "@/lib/format";
import { community, currentLedger, queue } from "@/lib/queries";

export const dynamic = "force-dynamic";

/**
 * 06 The queue. Members and public.
 *
 * Everything approved and waiting, each with a countdown.
 *
 * This page exists so the delay is a real opportunity to react rather than a
 * technicality. If nobody can see what is about to happen, a review period is
 * only a wait. Guardian cancellations appear here with their stated reason,
 * because a brake that can be pulled quietly is a brake nobody can be held to
 * account for.
 */
export default async function Queue({
  params,
}: {
  params: Promise<{ governor: string }>;
}) {
  const { governor } = await params;
  const c = community(governor);
  if (!c) notFound();

  const ledger = currentLedger();
  const all = queue(c.timelock);
  const waiting = all.filter((item) => item.state === "waiting" && item.ready_at > ledger);
  const ready = all.filter((item) => item.state === "waiting" && item.ready_at <= ledger);
  const done = all.filter((item) => item.state !== "waiting");

  return (
    <>
      <section className="masthead-block">
        <h1>The queue</h1>
        <p className="lede">
          Everything this community has approved and has not yet run. The delay
          is public and it is the window in which to object.
        </p>
      </section>

      <CommunityNav governor={c.governor} current="The queue" />

      <section>
        <div className="section-head">
          <h2>Ready to run</h2>
          <span className="note">Anyone may trigger these.</span>
        </div>
        {ready.length === 0 ? (
          <Empty>Nothing has finished its delay.</Empty>
        ) : (
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th>Action</th>
                  <th>Target</th>
                  <th>Since</th>
                  <th>Operation</th>
                </tr>
              </thead>
              <tbody>
                {ready.map((item) => (
                  <tr key={item.operation_id}>
                    <td className="mono">{item.fn || "call"}</td>
                    <td>
                      <Address value={item.target} />
                    </td>
                    <td>
                      <span className="chip chip--positive">Ready now</span>{" "}
                      <span className="note">
                        ledger {count(item.ready_at)}
                      </span>
                    </td>
                    <td>
                      <Hash value={item.operation_id} />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      <section>
        <div className="section-head">
          <h2>Waiting</h2>
          <span className="note">
            {count(waiting.length)} item{waiting.length === 1 ? "" : "s"}
          </span>
        </div>

        {waiting.length === 0 ? (
          <Empty>Nothing is waiting out a delay.</Empty>
        ) : (
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th>Action</th>
                  <th>Target</th>
                  <th>Ready in</th>
                  <th>Operation</th>
                </tr>
              </thead>
              <tbody>
                {waiting.map((item) => (
                  <tr key={item.operation_id}>
                    <td className="mono">{item.fn || "call"}</td>
                    <td>
                      <Address value={item.target} />
                    </td>
                    <td>
                      <Countdown ledgers={item.ready_at - ledger} />
                    </td>
                    <td>
                      <Hash value={item.operation_id} />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        <p className="note" style={{ marginTop: "var(--s-3)" }}>
          A countdown here is in ledgers, which is what the contract counts. The
          time beside it is an estimate: ledger time drifts and nobody controls
          it.
        </p>
      </section>

      <section>
        <div className="section-head">
          <h2>Already dealt with</h2>
        </div>

        {done.length === 0 ? (
          <Empty>Nothing has run or been stopped yet.</Empty>
        ) : (
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th>Action</th>
                  <th>What happened</th>
                  <th>Ledger</th>
                  <th>Reason given</th>
                </tr>
              </thead>
              <tbody>
                {done.map((item) => (
                  <tr key={item.operation_id}>
                    <td className="mono">{item.fn || "call"}</td>
                    <td>
                      {item.state === "executed" ? (
                        <span className="chip chip--outflow">Executed</span>
                      ) : (
                        <span className="chip chip--caution">Stopped</span>
                      )}
                    </td>
                    <td className="fig">
                      {count(item.executed_ledger ?? item.cancelled_ledger ?? 0)}
                    </td>
                    <td style={{ whiteSpace: "normal" }}>
                      {item.cancel_reason ? (
                        <>
                          {item.cancel_reason}
                          {item.cancelled_by ? (
                            <>
                              {" "}
                              <span className="note">
                                &mdash; <Address value={item.cancelled_by} />
                              </span>
                            </>
                          ) : null}
                        </>
                      ) : (
                        <LedgerLink tx={item.tx} />
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </>
  );
}
