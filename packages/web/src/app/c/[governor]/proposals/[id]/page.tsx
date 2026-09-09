import Link from "next/link";
import { notFound } from "next/navigation";

import {
  Address,
  CommunityNav,
  Countdown,
  Empty,
  Hash,
  LedgerLink,
  VoteBar,
} from "@/components/pieces";
import {
  amount,
  count,
  percent,
  proposalState,
  stateLabel,
  stateTone,
  when,
} from "@/lib/format";
import {
  community,
  currentLedger,
  liveVotingPower,
  policies,
  proposal,
  queue,
  tally,
  votes,
  windowSpent,
} from "@/lib/queries";

export const dynamic = "force-dynamic";

/**
 * 04 Proposal detail. Members and public.
 *
 * The proposal in plain words, the effect on the treasury, the vote totals
 * with quorum progress against live voting power, and the full timeline with
 * transaction links.
 *
 * This is the page the product is judged on, and the timeline is the half of
 * the story Stolla cannot tell: created, voting opened, voted, succeeded,
 * queued, delay elapsed, executed, funds sent.
 */
export default async function ProposalPage({
  params,
}: {
  params: Promise<{ governor: string; id: string }>;
}) {
  const { governor, id } = await params;
  const c = community(governor);
  if (!c) notFound();

  const p = proposal(id);
  if (!p || p.governor !== c.governor) notFound();

  const ledger = currentLedger();
  const t = tally(p.id);
  const cast = votes(p.id);
  const live = liveVotingPower(c, ledger);

  // The quorum the contract will apply. The index knows live power at the
  // snapshot; the exact share is the governor's, so this is shown as the
  // measure it is rather than as a promise.
  const state = proposalState(p, t, 0n, ledger);
  const reached = t.forVotes + t.abstainVotes;

  const actions = parseActions(p.actions);
  const spending = policies(c.treasury);
  const queued = queue(c.timelock).filter((item) => item.governor === c.governor);

  return (
    <>
      <section className="masthead-block">
        <span className="label">
          <Link href={`/c/${c.governor}`}>{c.name || "Unnamed community"}</Link>
        </span>
        <h1 style={{ maxWidth: "26ch" }}>{p.description || "Untitled proposal"}</h1>
        <div
          style={{
            display: "flex",
            gap: "var(--s-3)",
            alignItems: "center",
            flexWrap: "wrap",
            marginTop: "var(--s-3)",
          }}
        >
          <span className={`chip ${stateTone(state)}`}>{stateLabel(state)}</span>
          <span className="note">
            Proposed by <Address value={p.proposer} /> at ledger{" "}
            <span className="fig">{count(p.created_ledger)}</span>
          </span>
          <Hash value={p.id} />
        </div>
      </section>

      <CommunityNav governor={c.governor} current="Proposals" />

      <section>
        <div className="section-head">
          <h2>What this does</h2>
          <span className="note">
            {actions.length} call{actions.length === 1 ? "" : "s"}
          </span>
        </div>

        {actions.length === 0 ? (
          <Empty>The index has not recorded this proposal&rsquo;s calls yet.</Empty>
        ) : (
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th className="num">#</th>
                  <th>Contract</th>
                  <th>Call</th>
                  <th>Arguments</th>
                </tr>
              </thead>
              <tbody>
                {actions.map((action, i) => (
                  <tr key={i}>
                    <td className="num fig">{i + 1}</td>
                    <td>
                      <Address value={action.target} />
                      {action.target === c.treasury ? (
                        <span className="note"> the treasury</span>
                      ) : null}
                    </td>
                    <td className="mono">{action.fn}</td>
                    <td className="mono" style={{ whiteSpace: "normal" }}>
                      {action.args}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        <p className="note" style={{ marginTop: "var(--s-3)" }}>
          The description and the calls are hashed together, so the words and
          the action cannot drift apart after a vote.
        </p>
      </section>

      <section>
        <div className="section-head">
          <h2>What it would cost</h2>
        </div>

        {spending.length === 0 ? (
          <Empty>
            This treasury has no spending policy, so nothing can leave it
            whatever this proposal says.
          </Empty>
        ) : (
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th>Asset</th>
                  <th className="num">Most per payment</th>
                  <th className="num">Left in the window</th>
                  <th>Whether this fits</th>
                </tr>
              </thead>
              <tbody>
                {spending.map((policy) => {
                  const spent = windowSpent(c.treasury, policy, ledger);
                  const cap = BigInt(policy.window_cap || "0");
                  const left = cap > spent ? cap - spent : 0n;
                  const near = cap > 0n && (left * 100n) / cap < 20n;
                  return (
                    <tr key={policy.asset}>
                      <td>
                        <Address value={policy.asset} />
                      </td>
                      <td className="num fig">{amount(policy.per_payment_cap)}</td>
                      <td className="num fig">{amount(left.toString())}</td>
                      <td>
                        {near ? (
                          <span className="chip chip--caution">Near the limit</span>
                        ) : (
                          <span className="note">Inside the limits</span>
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
          <h2>The vote</h2>
          <span className="note">
            {count(t.voters)} member{t.voters === 1 ? "" : "s"} voted
          </span>
        </div>

        <div className="panel">
          <VoteBar
            forVotes={t.forVotes}
            againstVotes={t.againstVotes}
            abstainVotes={t.abstainVotes}
          />
          <div
            style={{
              display: "flex",
              gap: "var(--s-5)",
              marginTop: "var(--s-3)",
              flexWrap: "wrap",
            }}
          >
            <span>
              <span className="label" style={{ display: "inline", marginRight: 6 }}>
                For
              </span>
              <span className="fig" style={{ color: "var(--positive)" }}>
                {count(t.forVotes.toString())}
              </span>
            </span>
            <span>
              <span className="label" style={{ display: "inline", marginRight: 6 }}>
                Against
              </span>
              <span className="fig" style={{ color: "var(--outflow)" }}>
                {count(t.againstVotes.toString())}
              </span>
            </span>
            <span>
              <span className="label" style={{ display: "inline", marginRight: 6 }}>
                Abstain
              </span>
              <span className="fig">{count(t.abstainVotes.toString())}</span>
            </span>
          </div>

          <div style={{ marginTop: "var(--s-5)" }}>
            <span className="label">Quorum progress</span>
            <div className="bar">
              <span
                className="bar__reached"
                style={{
                  width:
                    live === 0n
                      ? "0%"
                      : `${Math.min((Number(reached) / Number(live)) * 100, 100)}%`,
                }}
              />
            </div>
            <p className="note" style={{ marginTop: "var(--s-2)" }}>
              <span className="fig">{count(reached.toString())}</span> of{" "}
              <span className="fig">{count(live.toString())}</span> live voting
              power has taken part, {live === 0n ? "—" : percent(Number(reached), Number(live))}.
              Quorum is measured against power that has not lapsed, so it
              reflects members who are actually present.
            </p>
          </div>
        </div>
      </section>

      <section>
        <div className="section-head">
          <h2>The timeline</h2>
          <span className="note">Every stage, linked to the ledger.</span>
        </div>

        <ol className="timeline">
          <Stage
            done
            stage="Created"
            ledger={p.created_ledger}
            ts={p.created_at}
            tx={p.created_tx}
          />
          <Stage
            done={ledger > p.snapshot}
            stage="Voting opened"
            ledger={p.snapshot + 1}
            note={
              ledger <= p.snapshot ? (
                <Countdown ledgers={p.snapshot + 1 - ledger} />
              ) : undefined
            }
          />
          <Stage
            done={ledger > p.deadline}
            stage="Voting closed"
            ledger={p.deadline}
            note={
              ledger <= p.deadline ? (
                <Countdown ledgers={p.deadline - ledger} />
              ) : undefined
            }
          />
          <Stage
            done={p.queued_ledger !== null}
            stage="Queued"
            ledger={p.queued_ledger ?? undefined}
            tx={p.queued_tx ?? undefined}
            note={
              p.queued_ledger === null ? (
                <span className="note">
                  An approved proposal has to be queued before it can run.
                </span>
              ) : undefined
            }
          />
          <Stage
            done={p.eta !== null && ledger >= p.eta}
            stage="Delay elapsed"
            ledger={p.eta ?? undefined}
            note={
              p.eta !== null && ledger < p.eta ? (
                <Countdown ledgers={p.eta - ledger} />
              ) : undefined
            }
          />
          <Stage
            done={p.executed_ledger !== null}
            paid={p.executed_ledger !== null}
            stage="Executed, funds sent"
            ledger={p.executed_ledger ?? undefined}
            tx={p.executed_tx ?? undefined}
            note={
              p.executed_ledger === null ? (
                <span className="note">
                  Execution is permissionless once the delay has passed. Anyone
                  can finish the job.
                </span>
              ) : undefined
            }
          />
          {p.cancelled_ledger !== null ? (
            <li data-done="true">
              <div className="timeline__stage" style={{ color: "var(--outflow)" }}>
                Stopped
              </div>
              <div className="timeline__meta">
                <span>ledger {count(p.cancelled_ledger)}</span>
                <LedgerLink tx={p.cancelled_tx} />
              </div>
              {p.cancel_reason ? (
                <div className="notice notice--outflow" style={{ marginTop: "var(--s-2)" }}>
                  <strong>Reason given:</strong> {p.cancel_reason}
                  {p.cancelled_by ? (
                    <>
                      {" "}
                      <span className="note">
                        by <Address value={p.cancelled_by} />
                      </span>
                    </>
                  ) : null}
                </div>
              ) : null}
            </li>
          ) : null}
        </ol>

        {queued.length > 0 ? (
          <p className="note" style={{ marginTop: "var(--s-4)" }}>
            The <Link href={`/c/${c.governor}/queue`}>queue</Link> shows every
            approved action in this community waiting out its delay.
          </p>
        ) : null}
      </section>

      <section>
        <div className="section-head">
          <h2>Who voted</h2>
        </div>

        {cast.length === 0 ? (
          <Empty>Nobody has voted on this proposal.</Empty>
        ) : (
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th>Member</th>
                  <th>Vote</th>
                  <th className="num">Weight</th>
                  <th>Reason</th>
                  <th>Ledger</th>
                </tr>
              </thead>
              <tbody>
                {cast.map((vote) => (
                  <tr key={`${vote.proposal_id}-${vote.voter}`}>
                    <td>
                      <Address value={vote.voter} />
                    </td>
                    <td>
                      <span
                        className={`chip ${
                          vote.vote_type === 1
                            ? "chip--positive"
                            : vote.vote_type === 0
                              ? "chip--outflow"
                              : "chip--quiet"
                        }`}
                      >
                        {vote.vote_type === 1
                          ? "For"
                          : vote.vote_type === 0
                            ? "Against"
                            : "Abstain"}
                      </span>
                    </td>
                    <td className="num fig">{count(vote.weight)}</td>
                    <td style={{ whiteSpace: "normal" }}>
                      {vote.reason || <span className="note">—</span>}
                    </td>
                    <td>
                      <LedgerLink tx={vote.tx} label={String(vote.ledger)} />
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

function Stage({
  stage,
  ledger,
  ts,
  tx,
  done,
  paid,
  note,
}: {
  stage: string;
  ledger?: number;
  ts?: number;
  tx?: string;
  done?: boolean;
  paid?: boolean;
  note?: React.ReactNode;
}) {
  return (
    <li data-done={done ? "true" : "false"} data-paid={paid ? "true" : "false"}>
      <div className="timeline__stage">{stage}</div>
      <div className="timeline__meta">
        {ledger ? <span>ledger {count(ledger)}</span> : null}
        {ts ? <span>{when(ts)}</span> : null}
        {tx ? <LedgerLink tx={tx} /> : null}
        {note}
      </div>
    </li>
  );
}

/** Reads the calls the indexer recorded, without trusting their shape. */
function parseActions(raw: string): Array<{ target: string; fn: string; args: string }> {
  try {
    const parsed = JSON.parse(raw) as {
      targets?: unknown;
      functions?: unknown;
      args?: unknown;
    };
    const targets = Array.isArray(parsed.targets) ? parsed.targets : [];
    const functions = Array.isArray(parsed.functions) ? parsed.functions : [];
    const args = Array.isArray(parsed.args) ? parsed.args : [];

    return targets.map((target, i) => ({
      target: String(target),
      fn: String(functions[i] ?? ""),
      args: JSON.stringify(args[i] ?? []),
    }));
  } catch {
    return [];
  }
}
