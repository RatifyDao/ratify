import Link from "next/link";

import { Empty, NoIndex, Stat } from "@/components/pieces";
import { amount, count, percent } from "@/lib/format";
import { indexState } from "@/lib/index";
import {
  communities,
  currentLedger,
  executedCount,
  headline,
  memberCount,
  paidOut,
  turnout,
} from "@/lib/queries";

export const dynamic = "force-dynamic";

/**
 * 01 Landing. Public.
 *
 * One screen stating what this is, with live numbers underneath rather than
 * demo data dressed up as live. If the numbers are small, they are shown
 * small. Stolla labels a hardcoded panel as live and had to file an issue
 * against itself to correct it; the fix here is that there is no panel that
 * could be hardcoded, only a query.
 */
export default function Landing() {
  const state = indexState();
  const ledger = currentLedger();
  const numbers = headline(ledger);
  const all = communities();

  const paid = all.reduce((total, c) => total + paidOut(c.treasury), 0n);
  const medianTurnout =
    numbers.medianTurnout === null ? "—" : percent(numbers.medianTurnout, 1);

  return (
    <>
      <section className="masthead-block">
        <h1 style={{ fontSize: "var(--step-5)", maxWidth: "20ch" }}>
          The vote moves the money.
        </h1>
        <p className="lede">
          A proposal that passes is queued, not executed. It waits a fixed delay
          that every member can see and object during. When the delay ends,
          anyone can trigger it, and the treasury pays out inside limits the
          contract enforces rather than the operator.
        </p>
        <p className="lede">
          Delegated power carries a term and lapses unless it is renewed, so a
          quorum reflects the people who are actually present.
        </p>
        <div style={{ display: "flex", gap: "var(--s-3)", marginTop: "var(--s-5)" }}>
          <Link className="button" href="/communities">
            See the communities
          </Link>
        </div>
      </section>

      {!state.present ? <NoIndex /> : null}

      <section>
        <div className="section-head">
          <h2>Where things stand</h2>
          <span className="note">
            Counted from the index at ledger {count(ledger)}.
          </span>
        </div>

        <div className="grid">
          <Stat
            label="Communities governed"
            value={count(numbers.communities)}
            note={numbers.communities === 0 ? "None deployed yet." : undefined}
          />
          <Stat
            label="Proposals executed"
            value={count(numbers.executed)}
            note={
              numbers.executed === 0
                ? "No decision has been carried through yet."
                : undefined
            }
          />
          <Stat
            label="Paid out of treasuries"
            value={amount(paid.toString())}
            tone={paid > 0n ? "outflow" : "quiet"}
            note={`${count(numbers.paidOut)} payment${numbers.paidOut === 1 ? "" : "s"}`}
          />
          <Stat
            label="Median turnout"
            value={medianTurnout}
            note={
              numbers.medianTurnout === null
                ? "No community has closed a vote yet."
                : "Across the last five proposals of each community."
            }
          />
        </div>
      </section>

      <section>
        <div className="section-head">
          <h2>Communities</h2>
          <Link href="/communities" className="note">
            The full register
          </Link>
        </div>

        {all.length === 0 ? (
          <Empty>
            No community has been deployed to this factory yet. The register is
            empty, and this page says so rather than showing an example.
          </Empty>
        ) : (
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th>Community</th>
                  <th className="num">Members</th>
                  <th className="num">Turnout</th>
                  <th className="num">Executed</th>
                </tr>
              </thead>
              <tbody>
                {all.slice(0, 5).map((c) => {
                  const t = turnout(c, ledger);
                  const executed = executedCount(c.governor);
                  return (
                    <tr key={c.governor}>
                      <td>
                        <Link href={`/c/${c.governor}`}>{c.name || "Unnamed"}</Link>
                      </td>
                      <td className="num fig">{count(memberCount(c.membership))}</td>
                      <td className="num fig">
                        {t === null ? (
                          <span className="fig--none">unknown</span>
                        ) : (
                          percent(t, 1)
                        )}
                      </td>
                      <td className="num fig">
                        {executed === 0 ? (
                          <span className="fig--none">nothing</span>
                        ) : (
                          count(executed)
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </section>

      <section>
        <div className="section-head">
          <h2>What the contracts guarantee</h2>
        </div>
        <div className="grid">
          <div className="panel">
            <h3>A vote ends in an action</h3>
            <p className="note">
              A passed proposal is queued and waits a fixed delay. Execution
              afterwards is permissionless, so no operator is needed to finish
              the job. Executing twice is impossible, and executing early is
              impossible.
            </p>
          </div>
          <div className="panel">
            <h3>Delegated power expires</h3>
            <p className="note">
              A grant carries a term and lapses unless renewed. Lapsed power
              stops counting towards a delegate&rsquo;s weight and towards
              quorum, and anyone may sweep an expired grant.
            </p>
          </div>
          <div className="panel">
            <h3>Delegates carry a record</h3>
            <p className="note">
              How often a delegate votes, and whether they turned up for the
              proposals that were close. Written by the governance contracts
              themselves, so neither the delegate nor this interface can edit
              it.
            </p>
          </div>
        </div>
      </section>
    </>
  );
}
