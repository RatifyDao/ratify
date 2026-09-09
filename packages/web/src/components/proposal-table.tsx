import Link from "next/link";

import { count, proposalState, stateLabel, stateTone } from "@/lib/format";
import { tally } from "@/lib/queries";

/**
 * A list of proposals, as the home page and the proposals page both show it.
 *
 * State is derived here rather than stored, because a proposal's state depends
 * on where the ledger has got to as well as on what happened. Two pages
 * showing the same proposal in different states would be the interface
 * disagreeing with itself.
 */
export function ProposalTable({
  governor,
  list,
  ledger,
}: {
  governor: string;
  list: Array<{
    id: string;
    description: string;
    snapshot: number;
    deadline: number;
    queued_ledger: number | null;
    executed_ledger: number | null;
    cancelled_ledger: number | null;
  }>;
  ledger: number;
}) {
  return (
    <div className="table-scroll">
      <table>
        <thead>
          <tr>
            <th>Proposal</th>
            <th>State</th>
            <th className="num">For</th>
            <th className="num">Against</th>
          </tr>
        </thead>
        <tbody>
          {list.map((p) => {
            const t = tally(p.id);
            const state = proposalState(
              {
                snapshot: p.snapshot,
                deadline: p.deadline,
                executed_ledger: p.executed_ledger,
                cancelled_ledger: p.cancelled_ledger,
                queued_ledger: p.queued_ledger,
              },
              t,
              0n,
              ledger,
            );
            return (
              <tr key={p.id}>
                <td>
                  <Link href={`/c/${governor}/proposals/${p.id}`}>
                    {p.description || "Untitled proposal"}
                  </Link>
                </td>
                <td>
                  <span className={`chip ${stateTone(state)}`}>{stateLabel(state)}</span>
                </td>
                <td className="num fig">{count(t.forVotes.toString())}</td>
                <td className="num fig">{count(t.againstVotes.toString())}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
