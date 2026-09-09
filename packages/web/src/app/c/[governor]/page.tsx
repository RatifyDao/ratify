import Link from "next/link";
import { notFound } from "next/navigation";

import { CommunityNav, Countdown, Empty, Stat } from "@/components/pieces";
import { ProposalTable } from "@/components/proposal-table";
import { amount, count, percent } from "@/lib/format";
import {
  community,
  currentLedger,
  executedCount,
  liveVotingPower,
  memberCount,
  paidOut,
  policies,
  proposals,
  turnout,
  waitingQueue,
  windowSpent,
} from "@/lib/queries";

export const dynamic = "force-dynamic";

/**
 * 03 Community home. Members and public.
 *
 * The state of the union. Treasury balance and what the spending policy
 * permits, proposals currently open, what is sitting in the queue with a
 * countdown, and turnout on recent votes.
 *
 * A visitor should understand the health of this community in ten seconds,
 * which is why turnout and the queue are above the fold and not behind a tab.
 */
export default async function CommunityHome({
  params,
}: {
  params: Promise<{ governor: string }>;
}) {
  const { governor } = await params;
  const c = community(governor);
  if (!c) notFound();

  const ledger = currentLedger();
  const all = proposals(c.governor, 50);
  const open = all.filter((p) => p.snapshot < ledger && p.deadline >= ledger);
  const waiting = waitingQueue(c.timelock);
  const recent = all.slice(0, 6);
  const t = turnout(c, ledger);
  const live = liveVotingPower(c, ledger);
  const paid = paidOut(c.treasury);
  const spending = policies(c.treasury);

  return (
    <>
      <section className="masthead-block">
        <h1>{c.name || "Unnamed community"}</h1>
        <p className="lede">
          {executedCount(c.governor) === 0
            ? "This community has not executed a proposal yet."
            : `${count(executedCount(c.governor))} proposal${executedCount(c.governor) === 1 ? " has" : "s have"} been carried through to a payment.`}
        </p>
      </section>

      <CommunityNav governor={c.governor} current="Home" />

      <section>
        <div className="grid">
          <Stat
            label="Members"
            value={count(memberCount(c.membership))}
            note={`${count(live.toString())} units of live voting power`}
          />
          <Stat
            label="Turnout"
            value={t === null ? "unknown" : percent(t, 1)}
            tone={t === null ? "quiet" : undefined}
            note={
              t === null
                ? "No proposal has closed yet."
                : "Last five closed proposals."
            }
          />
          <Stat
            label="Paid out"
            value={paid === 0n ? "nothing" : amount(paid.toString())}
            tone={paid === 0n ? "quiet" : "outflow"}
            note="Every payment is listed on the treasury page."
          />
          <Stat
            label="In the queue"
            value={count(waiting.length)}
            note={
              waiting.length === 0
                ? "Nothing is waiting."
                : "Approved, waiting out the delay."
            }
          />
        </div>
      </section>

      <section>
        <div className="section-head">
          <h2>Waiting in the queue</h2>
          <Link href={`/c/${c.governor}/queue`} className="note">
            The queue
          </Link>
        </div>

        {waiting.length === 0 ? (
          <Empty>Nothing is waiting. An approved proposal appears here first.</Empty>
        ) : (
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th>Action</th>
                  <th>Ready in</th>
                </tr>
              </thead>
              <tbody>
                {waiting.map((item) => (
                  <tr key={item.operation_id}>
                    <td>
                      <span className="mono">{item.fn || "call"}</span>{" "}
                      <span className="note">on</span>{" "}
                      <span className="mono" title={item.target}>
                        {item.target.slice(0, 6)}…{item.target.slice(-6)}
                      </span>
                    </td>
                    <td>
                      <Countdown ledgers={item.ready_at - ledger} />
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
          <h2>Open for voting</h2>
          <Link href={`/c/${c.governor}/propose`} className="note">
            Make a proposal
          </Link>
        </div>

        {open.length === 0 ? (
          <Empty>No proposal is open for voting.</Empty>
        ) : (
          <ProposalTable governor={c.governor} list={open} ledger={ledger} />
        )}
      </section>

      <section>
        <div className="section-head">
          <h2>What the treasury may spend</h2>
          <Link href={`/c/${c.governor}/treasury`} className="note">
            The treasury
          </Link>
        </div>

        {spending.length === 0 ? (
          <Empty>
            No asset has a spending policy, so nothing can leave this treasury.
            Setting one is a proposal.
          </Empty>
        ) : (
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th>Asset</th>
                  <th className="num">Most per payment</th>
                  <th className="num">Window cap</th>
                  <th className="num">Left in the window</th>
                </tr>
              </thead>
              <tbody>
                {spending.map((policy) => {
                  const spent = windowSpent(c.treasury, policy, ledger);
                  const cap = BigInt(policy.window_cap || "0");
                  const left = cap > spent ? cap - spent : 0n;
                  return (
                    <tr key={policy.asset}>
                      <td className="mono" title={policy.asset}>
                        {policy.asset.slice(0, 6)}…{policy.asset.slice(-6)}
                      </td>
                      <td className="num fig">{amount(policy.per_payment_cap)}</td>
                      <td className="num fig">{amount(policy.window_cap)}</td>
                      <td className="num fig">{amount(left.toString())}</td>
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
          <h2>Recent proposals</h2>
          <Link href={`/c/${c.governor}/proposals`} className="note">
            All proposals
          </Link>
        </div>
        {recent.length === 0 ? (
          <Empty>Nothing has been proposed here yet.</Empty>
        ) : (
          <ProposalTable governor={c.governor} list={recent} ledger={ledger} />
        )}
      </section>
    </>
  );
}
