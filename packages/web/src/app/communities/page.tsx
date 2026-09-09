import Link from "next/link";

import { Address, Empty, NoIndex } from "@/components/pieces";
import { amount, count, date, percent } from "@/lib/format";
import { indexState } from "@/lib/index";
import {
  communities,
  currentLedger,
  executedCount,
  liveVotingPower,
  memberCount,
  paidOut,
  turnout,
} from "@/lib/queries";

export const dynamic = "force-dynamic";

export const metadata = { title: "Communities" };

/**
 * 02 Community directory. Public.
 *
 * Every community on the register, with treasury size, member count, turnout
 * over the last five proposals, and whether the community has executed
 * anything.
 *
 * Turnout is shown because it is the honest measure of whether a DAO is alive,
 * and because a directory that shows only treasury size rewards the
 * communities with the most money rather than the ones that work.
 */
export default function Directory() {
  const state = indexState();
  const ledger = currentLedger();
  const all = communities();

  return (
    <>
      <section className="masthead-block">
        <h1>Communities</h1>
        <p className="lede">
          Every community on the register. Turnout is the share of live voting
          power that took part in the last five closed proposals, and a
          community that has never closed one says so.
        </p>
      </section>

      {!state.present ? <NoIndex /> : null}

      {all.length === 0 ? (
        <Empty>
          The register is empty. Nothing has been deployed to this factory yet.
        </Empty>
      ) : (
        <div className="table-scroll">
          <table>
            <thead>
              <tr>
                <th>Community</th>
                <th className="num">Members</th>
                <th className="num">Live power</th>
                <th className="num">Turnout</th>
                <th className="num">Executed</th>
                <th className="num">Paid out</th>
                <th>Governor</th>
                <th className="num">Deployed</th>
              </tr>
            </thead>
            <tbody>
              {all.map((c) => {
                const t = turnout(c, ledger);
                const executed = executedCount(c.governor);
                const paid = paidOut(c.treasury);
                return (
                  <tr key={c.governor}>
                    <td>
                      <Link href={`/c/${c.governor}`}>{c.name || "Unnamed"}</Link>
                    </td>
                    <td className="num fig">{count(memberCount(c.membership))}</td>
                    <td className="num fig">
                      {count(liveVotingPower(c, ledger).toString())}
                    </td>
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
                    <td className="num fig">
                      {paid === 0n ? (
                        <span className="fig--none">nothing</span>
                      ) : (
                        <span className="fig--outflow">{amount(paid.toString())}</span>
                      )}
                    </td>
                    <td>
                      <Address value={c.governor} />
                    </td>
                    <td className="num">
                      <span className="note">
                        {c.deployed_at ? `ledger ${count(c.deployed_at)}` : "—"}
                      </span>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      <p className="note" style={{ marginTop: "var(--s-4)" }}>
        A community that has executed nothing has never carried a decision
        through to its consequence. That is worth knowing before you join one,
        so it is a column rather than something you have to go looking for.
      </p>
    </>
  );
}
