import Link from "next/link";
import { notFound } from "next/navigation";

import { Address, CommunityNav, Empty, LedgerLink, Stat } from "@/components/pieces";
import { amount, count, duration, when } from "@/lib/format";
import {
  community,
  currentLedger,
  outflows,
  paidOut,
  policies,
  windowSpent,
} from "@/lib/queries";

export const dynamic = "force-dynamic";

/**
 * 07 Treasury. Members and public.
 *
 * The policy limits currently in force, remaining headroom in the rolling
 * window, and every outflow ever made with the proposal that authorised it.
 *
 * A public account of the money. The one strong colour in the interface marks
 * funds leaving a treasury, and it is used here and nowhere else.
 */
export default async function Treasury({
  params,
}: {
  params: Promise<{ governor: string }>;
}) {
  const { governor } = await params;
  const c = community(governor);
  if (!c) notFound();

  const ledger = currentLedger();
  const spending = policies(c.treasury);
  const payments = outflows(c.treasury);
  const paid = paidOut(c.treasury);

  return (
    <>
      <section className="masthead-block">
        <h1>Treasury</h1>
        <p className="lede">
          What this community may spend, what it has left inside its rolling
          window, and every payment it has ever made against the proposal that
          authorised it.
        </p>
        <p className="note">
          Held at <Address value={c.treasury} keep={10} />. It takes instructions
          from the timelock and from nobody else, and it refuses any that breach
          the policy below whatever a vote said.
        </p>
      </section>

      <CommunityNav governor={c.governor} current="Treasury" />

      <section>
        <div className="grid">
          <Stat
            label="Paid out, all time"
            value={paid === 0n ? "nothing" : amount(paid.toString())}
            tone={paid === 0n ? "quiet" : "outflow"}
            note={`${count(payments.length)} payment${payments.length === 1 ? "" : "s"}`}
          />
          <Stat
            label="Assets that may leave"
            value={count(spending.length)}
            note={
              spending.length === 0
                ? "Nothing can leave this treasury."
                : "An asset with no policy cannot leave."
            }
          />
        </div>
      </section>

      <section>
        <div className="section-head">
          <h2>The policy in force</h2>
          <span className="note">Changing it is a proposal.</span>
        </div>

        {spending.length === 0 ? (
          <Empty>
            No asset has a policy, so nothing can leave this treasury at all.
            That is the safe state, not a broken one.
          </Empty>
        ) : (
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th>Asset</th>
                  <th className="num">Most per payment</th>
                  <th className="num">Window cap</th>
                  <th>Window</th>
                  <th className="num">Spent in it</th>
                  <th className="num">Headroom</th>
                </tr>
              </thead>
              <tbody>
                {spending.map((policy) => {
                  const spent = windowSpent(c.treasury, policy, ledger);
                  const cap = BigInt(policy.window_cap || "0");
                  const left = cap > spent ? cap - spent : 0n;
                  const tight = cap > 0n && (left * 100n) / cap < 20n;
                  return (
                    <tr key={policy.asset}>
                      <td>
                        <Address value={policy.asset} />
                      </td>
                      <td className="num fig">{amount(policy.per_payment_cap)}</td>
                      <td className="num fig">{amount(policy.window_cap)}</td>
                      <td>
                        <span className="fig">{count(policy.window_ledgers)}</span>{" "}
                        ledgers{" "}
                        <span className="note">
                          ({duration(policy.window_ledgers)}, an estimate)
                        </span>
                      </td>
                      <td className="num fig">{amount(spent.toString())}</td>
                      <td className="num fig">
                        {tight ? (
                          <span style={{ color: "var(--caution)" }}>
                            {amount(left.toString())}
                          </span>
                        ) : (
                          amount(left.toString())
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}

        <p className="note" style={{ marginTop: "var(--s-3)" }}>
          Limits are printed here rather than hidden behind a hover, because a
          member deciding how to vote needs to know them without going looking.
        </p>
      </section>

      <section>
        <div className="section-head">
          <h2>Every payment ever made</h2>
          <span className="note">Oldest last.</span>
        </div>

        {payments.length === 0 ? (
          <Empty>
            Nothing has left this treasury. If that stays true, this community
            has not yet done the one thing a treasury is for.
          </Empty>
        ) : (
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th className="num">#</th>
                  <th>To</th>
                  <th>Asset</th>
                  <th className="num">Amount</th>
                  <th>Authorised by</th>
                  <th>When</th>
                </tr>
              </thead>
              <tbody>
                {payments.map((payment) => (
                  <tr key={`${payment.treasury}-${payment.idx}`}>
                    <td className="num fig">{payment.idx}</td>
                    <td>
                      <Address value={payment.recipient} />
                    </td>
                    <td>
                      <Address value={payment.asset} />
                    </td>
                    <td className="num fig fig--outflow">
                      {amount(payment.amount)}
                    </td>
                    <td>
                      <Link
                        href={`/c/${c.governor}/proposals/${payment.proposal_id}`}
                        className="mono"
                      >
                        {payment.proposal_id.slice(0, 8)}…
                      </Link>
                    </td>
                    <td>
                      <span className="note">{when(payment.ts)}</span>{" "}
                      <LedgerLink tx={payment.tx} />
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
