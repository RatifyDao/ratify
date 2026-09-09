import { notFound } from "next/navigation";

import { Address, CommunityNav, Countdown, Empty } from "@/components/pieces";
import { count, percent } from "@/lib/format";
import { community, currentLedger, delegates } from "@/lib/queries";

export const dynamic = "force-dynamic";

/**
 * 08 Delegates. Members and public.
 *
 * The delegate register, sorted by participation rather than by power. For
 * each one: voting power held, participation rate, record on contested
 * proposals, and when their grants begin to lapse.
 *
 * Sorting by participation is the whole argument of the page. A list sorted by
 * power tells you who has the most votes, which you can already see. This one
 * tells you who uses them.
 */
export default async function Delegates({
  params,
}: {
  params: Promise<{ governor: string }>;
}) {
  const { governor } = await params;
  const c = community(governor);
  if (!c) notFound();

  const ledger = currentLedger();
  const list = delegates(c, ledger);

  return (
    <>
      <section className="masthead-block">
        <h1>Delegates</h1>
        <p className="lede">
          Everyone holding voting power in this community, sorted by how often
          they use it. Every figure here is written by the governance contracts
          themselves, so neither the delegate nor this interface can edit it.
        </p>
      </section>

      <CommunityNav governor={c.governor} current="Delegates" />

      {list.length === 0 ? (
        <Empty>
          Nobody holds live voting power here. A member grants their power for a
          term, and until they do it counts for nothing.
        </Empty>
      ) : (
        <div className="table-scroll">
          <table>
            <thead>
              <tr>
                <th>Delegate</th>
                <th className="num">Power</th>
                <th className="num">Members</th>
                <th className="num">Turnout</th>
                <th className="num">On close votes</th>
                <th>First grant lapses</th>
              </tr>
            </thead>
            <tbody>
              {list.map((delegate) => (
                <tr key={delegate.account}>
                  <td>
                    <Address value={delegate.account} keep={8} />
                  </td>
                  <td className="num fig">{count(delegate.power.toString())}</td>
                  <td className="num fig">{count(delegate.heads)}</td>
                  <td className="num fig">
                    {delegate.eligible === 0 ? (
                      <span className="fig--none">unknown</span>
                    ) : (
                      <>
                        {percent(delegate.voted, delegate.eligible)}{" "}
                        <span className="note">
                          ({count(delegate.voted)}/{count(delegate.eligible)})
                        </span>
                      </>
                    )}
                  </td>
                  <td className="num fig">
                    {delegate.contestedEligible === 0 ? (
                      <span className="fig--none">unknown</span>
                    ) : (
                      <>
                        {percent(delegate.contestedVoted, delegate.contestedEligible)}{" "}
                        <span className="note">
                          ({count(delegate.contestedVoted)}/
                          {count(delegate.contestedEligible)})
                        </span>
                      </>
                    )}
                  </td>
                  <td>
                    <Countdown ledgers={delegate.soonestLapse - ledger} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <section style={{ marginTop: "var(--s-6)" }}>
        <div className="section-head">
          <h2>How these figures are made</h2>
        </div>
        <div className="grid">
          <div className="panel">
            <h3>Turnout</h3>
            <p className="note">
              Proposals the delegate held power on, against the ones they voted
              on. A proposal counts once anyone settles it against the delegate,
              which anyone may do after voting closes and nobody can prevent.
            </p>
          </div>
          <div className="panel">
            <h3>Close votes</h3>
            <p className="note">
              A proposal decided by a narrow margin. Turning up for the easy ones
              and missing the close ones is what a single participation rate
              would hide, so it is counted separately.
            </p>
          </div>
          <div className="panel">
            <h3>Unknown, not zero</h3>
            <p className="note">
              A delegate nobody has settled yet reads as unknown. A rate out of
              nothing is not nought percent, and showing it as nought would
              punish a new delegate for the absence of a record rather than for
              a bad one.
            </p>
          </div>
        </div>
      </section>
    </>
  );
}
