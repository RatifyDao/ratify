import Link from "next/link";
import { notFound } from "next/navigation";

import { CommunityNav, Empty } from "@/components/pieces";
import { ProposalTable } from "@/components/proposal-table";
import { count } from "@/lib/format";
import { community, currentLedger, proposals } from "@/lib/queries";

export const dynamic = "force-dynamic";

/**
 * Every proposal a community has ever made.
 *
 * This page is why the indexer exists. Stolla scans events from the browser,
 * so its history is capped by what an RPC provider keeps, and it built a
 * module to label the data current, delayed, stale or unavailable. Here a
 * proposal from two years ago loads exactly like one from this morning,
 * because the index kept it.
 */
export default async function Proposals({
  params,
}: {
  params: Promise<{ governor: string }>;
}) {
  const { governor } = await params;
  const c = community(governor);
  if (!c) notFound();

  const ledger = currentLedger();
  const all = proposals(c.governor, 500);

  return (
    <>
      <section className="masthead-block">
        <h1>Proposals</h1>
        <p className="lede">
          {all.length === 0
            ? "Nothing has been proposed here yet."
            : `${count(all.length)} proposal${all.length === 1 ? "" : "s"}, oldest last. Governance history here is permanent, not bounded by what an RPC provider keeps.`}
        </p>
      </section>

      <CommunityNav governor={c.governor} current="Proposals" />

      {all.length === 0 ? (
        <Empty>
          Nothing has been proposed here yet.{" "}
          <Link href={`/c/${c.governor}/propose`}>Make the first proposal</Link>.
        </Empty>
      ) : (
        <ProposalTable governor={c.governor} list={all} ledger={ledger} />
      )}
    </>
  );
}
