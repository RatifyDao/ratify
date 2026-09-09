import { notFound } from "next/navigation";

import { Address, CommunityNav } from "@/components/pieces";
import { MembershipPanel } from "@/components/membership-panel";
import { count } from "@/lib/format";
import { community, currentLedger, liveVotingPower, memberCount } from "@/lib/queries";

export const dynamic = "force-dynamic";

/**
 * 09 Your membership. Members.
 *
 * Your tokens, your voting power, whether you have delegated and to whom, and
 * exactly when that grant expires. One action to renew, extend or withdraw.
 *
 * This page is why lapsing does not become an annoyance. A term that catches
 * a member by surprise is a bad design; a term with a countdown, a date and a
 * single button is a routine.
 */
export default async function Membership({
  params,
}: {
  params: Promise<{ governor: string }>;
}) {
  const { governor } = await params;
  const c = community(governor);
  if (!c) notFound();

  const ledger = currentLedger();

  return (
    <>
      <section className="masthead-block">
        <h1>Your membership</h1>
        <p className="lede">
          What you hold in {c.name || "this community"}, what it is worth, and
          when it lapses.
        </p>
        <p className="note">
          Membership tokens are credentials, not assets. They cannot be
          transferred or sold, which is what stops a vote from being something
          you can buy.
        </p>
      </section>

      <CommunityNav governor={c.governor} current="Your membership" />

      <MembershipPanel membership={c.membership} ledger={ledger} />

      <section style={{ marginTop: "var(--s-6)" }}>
        <div className="section-head">
          <h2>Where your power sits in the community</h2>
        </div>
        <div className="grid">
          <div className="panel">
            <h3>Live voting power</h3>
            <p className="note">
              <span className="fig">
                {count(liveVotingPower(c, ledger).toString())}
              </span>{" "}
              across{" "}
              <span className="fig">{count(memberCount(c.membership))}</span>{" "}
              members. Quorum is measured against this, not against every token
              ever issued.
            </p>
          </div>
          <div className="panel">
            <h3>The membership contract</h3>
            <p className="note">
              <Address value={c.membership} keep={10} />. Grants live here, and
              so does the term on each one.
            </p>
          </div>
          <div className="panel">
            <h3>Anyone may sweep a lapsed grant</h3>
            <p className="note">
              Once a term ends, any account may call the contract to take the
              power out of the live total. It needs nobody&rsquo;s permission and
              gains the caller nothing, which is what makes a lapse enforceable
              rather than a matter of good manners.
            </p>
          </div>
        </div>
      </section>
    </>
  );
}
