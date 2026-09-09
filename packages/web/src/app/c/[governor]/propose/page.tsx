import { notFound } from "next/navigation";

import { Address, CommunityNav, Empty } from "@/components/pieces";
import { ProposeForm } from "@/components/propose-form";
import { amount, count, duration } from "@/lib/format";
import { community, currentLedger, policies, windowSpent } from "@/lib/queries";

export const dynamic = "force-dynamic";

/**
 * 05 New proposal. Members above the threshold.
 *
 * A structured form, not a text box, and the limits printed beside it rather
 * than left to be discovered. The form shows in real time whether the
 * proposal would breach a policy limit, and refuses to submit one that could
 * never execute.
 */
export default async function Propose({
  params,
}: {
  params: Promise<{ governor: string }>;
}) {
  const { governor } = await params;
  const c = community(governor);
  if (!c) notFound();

  const ledger = currentLedger();
  const spending = policies(c.treasury);

  return (
    <>
      <section className="masthead-block">
        <h1>Make a proposal</h1>
        <p className="lede">
          The first release restricts proposals to treasury payments.
          Unrestricted contract calls arrive after an audit, not before one.
        </p>
      </section>

      <CommunityNav governor={c.governor} current="Proposals" />

      <div
        style={{
          display: "grid",
          gap: "var(--s-5)",
          gridTemplateColumns: "minmax(0, 1.4fr) minmax(0, 1fr)",
          alignItems: "start",
        }}
      >
        <ProposeForm governor={c.governor} treasury={c.treasury} />

        <aside>
          <div className="panel">
            <h3>What this treasury allows</h3>
            {spending.length === 0 ? (
              <Empty>
                No asset has a policy, so no payment can be proposed that would
                execute.
              </Empty>
            ) : (
              <table>
                <thead>
                  <tr>
                    <th>Asset</th>
                    <th className="num">Per payment</th>
                    <th className="num">Headroom</th>
                  </tr>
                </thead>
                <tbody>
                  {spending.map((policy) => {
                    const spent = windowSpent(c.treasury, policy, ledger);
                    const cap = BigInt(policy.window_cap || "0");
                    const left = cap > spent ? cap - spent : 0n;
                    return (
                      <tr key={policy.asset}>
                        <td>
                          <Address value={policy.asset} keep={4} />
                        </td>
                        <td className="num fig">{amount(policy.per_payment_cap)}</td>
                        <td className="num fig">{amount(left.toString())}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            )}
          </div>

          <div className="panel" style={{ marginTop: "var(--s-4)" }}>
            <h3>What happens next</h3>
            <ol className="note" style={{ paddingLeft: "1.1rem", margin: 0 }}>
              <li>Voting opens after the community&rsquo;s voting delay.</li>
              <li>Members vote for the voting period.</li>
              <li>
                If it carries and reaches quorum, anyone can queue it into the
                timelock.
              </li>
              <li>
                It waits out the delay in public, where anyone can object and the
                guardian can stop it with a stated reason.
              </li>
              <li>Anyone executes it, and the treasury pays.</li>
            </ol>
            {spending[0] ? (
              <p className="note" style={{ marginTop: "var(--s-3)" }}>
                The rolling window on{" "}
                <Address value={spending[0].asset} keep={4} /> is{" "}
                <span className="fig">{count(spending[0].window_ledgers)}</span>{" "}
                ledgers, about {duration(spending[0].window_ledgers)}.
              </p>
            ) : null}
          </div>
        </aside>
      </div>
    </>
  );
}
