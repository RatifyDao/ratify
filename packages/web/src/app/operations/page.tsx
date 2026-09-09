import { Empty, NoIndex } from "@/components/pieces";
import { count, when } from "@/lib/format";
import { indexPath, indexState, rows } from "@/lib/index";
import { communities, currentLedger, queue } from "@/lib/queries";

export const dynamic = "force-dynamic";

export const metadata = { title: "Operations" };

/**
 * 10 Operations. Staff.
 *
 * Plain and internal. Indexer lag, failed executions, queue health and
 * registry state.
 *
 * Internal tools should look internal. No stat cards, no reassurance, and the
 * numbers that are bad shown as they are. This is also the only page in RatifyDAO
 * that talks about indexer freshness: a member reading a proposal should not
 * have to think about it, which is the whole reason the indexer exists.
 */
export default function Operations() {
  const state = indexState();
  const ledger = currentLedger();
  const all = communities();

  const recent = rows<{
    id: string;
    ledger: number;
    ts: number;
    contract: string;
    topic: string;
    tx: string;
  }>("SELECT id, ledger, ts, contract, topic, tx FROM events ORDER BY ledger DESC LIMIT 40");

  const byTopic = rows<{ topic: string; n: number }>(
    "SELECT topic, COUNT(*) AS n FROM events GROUP BY topic ORDER BY n DESC",
  );

  const stuck = all.flatMap((c) =>
    queue(c.timelock)
      .filter((item) => item.state === "waiting" && item.ready_at <= ledger)
      .map((item) => ({ community: c.name || c.governor, item })),
  );

  return (
    <>
      <section className="masthead-block">
        <h1>Operations</h1>
        <p className="lede">Internal. The state of the index and the queues.</p>
      </section>

      {!state.present ? <NoIndex /> : null}

      <section>
        <div className="section-head">
          <h2>Index</h2>
        </div>
        <div className="table-scroll">
          <table>
            <tbody>
              <tr>
                <td>Store</td>
                <td className="mono" style={{ whiteSpace: "normal" }}>
                  {indexPath()}
                </td>
              </tr>
              <tr>
                <td>Present</td>
                <td>{state.present ? "yes" : "no"}</td>
              </tr>
              <tr>
                <td>Read up to ledger</td>
                <td className="fig">
                  {state.lastLedger === null ? "—" : count(state.lastLedger)}
                </td>
              </tr>
              <tr>
                <td>Events held</td>
                <td className="fig">{count(state.events)}</td>
              </tr>
              <tr>
                <td>First run</td>
                <td>{state.startedAt ? when(state.startedAt) : "—"}</td>
              </tr>
              <tr>
                <td>Communities followed</td>
                <td className="fig">{count(all.length)}</td>
              </tr>
            </tbody>
          </table>
        </div>
        <p className="note" style={{ marginTop: "var(--s-3)" }}>
          The chain is the source of truth and this is a read model. It can be
          emptied and rebuilt by replaying events, so losing it costs the time to
          replay and nothing else.
        </p>
      </section>

      <section>
        <div className="section-head">
          <h2>Ready and not executed</h2>
          <span className="note">
            {stuck.length === 0
              ? "Nothing waiting on an executor."
              : "Execution is permissionless. Anyone can clear these."}
          </span>
        </div>
        {stuck.length === 0 ? (
          <Empty>Every approved action has either run or is still waiting.</Empty>
        ) : (
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th>Community</th>
                  <th>Call</th>
                  <th className="num">Ready since</th>
                  <th className="num">Ledgers overdue</th>
                </tr>
              </thead>
              <tbody>
                {stuck.map(({ community, item }) => (
                  <tr key={item.operation_id}>
                    <td>{community}</td>
                    <td className="mono">{item.fn || "call"}</td>
                    <td className="num fig">{count(item.ready_at)}</td>
                    <td className="num fig" style={{ color: "var(--caution)" }}>
                      {count(ledger - item.ready_at)}
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
          <h2>Events by kind</h2>
        </div>
        {byTopic.length === 0 ? (
          <Empty>No events indexed.</Empty>
        ) : (
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th>Topic</th>
                  <th className="num">Count</th>
                </tr>
              </thead>
              <tbody>
                {byTopic.map((row) => (
                  <tr key={row.topic}>
                    <td className="mono">{row.topic}</td>
                    <td className="num fig">{count(row.n)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      <section>
        <div className="section-head">
          <h2>Last forty events</h2>
        </div>
        {recent.length === 0 ? (
          <Empty>Nothing has been indexed yet.</Empty>
        ) : (
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th className="num">Ledger</th>
                  <th>Topic</th>
                  <th>Contract</th>
                  <th>When</th>
                </tr>
              </thead>
              <tbody>
                {recent.map((event) => (
                  <tr key={event.id}>
                    <td className="num fig">{count(event.ledger)}</td>
                    <td className="mono">{event.topic}</td>
                    <td className="mono" title={event.contract}>
                      {event.contract.slice(0, 6)}…{event.contract.slice(-6)}
                    </td>
                    <td className="note">{when(event.ts)}</td>
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
