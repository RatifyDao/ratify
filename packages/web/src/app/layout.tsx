import type { Metadata } from "next";
import Link from "next/link";

import { WalletProvider } from "@/components/wallet";
import "@/styles/global.css";

/**
 * The three faces, fetched by the browser rather than at build time.
 *
 * Spectral for display and headings, a serif with institutional weight.
 * Inter for the interface and every figure, because it has a true tabular
 * set. IBM Plex Mono for addresses and hashes.
 *
 * Loaded with a stylesheet link rather than through the build, so building
 * this project never depends on reaching a font host. Each face has a real
 * fallback in the design tokens, and the pages are legible without any of
 * them.
 */
const FONTS =
  "https://fonts.googleapis.com/css2?family=Spectral:wght@400;500;600&family=Inter:wght@400;500;600&family=IBM+Plex+Mono:wght@400;500&display=swap";

export const metadata: Metadata = {
  title: {
    default: "RatifyDAO",
    template: "%s · RatifyDAO",
  },
  description:
    "Governance on Stellar where the vote actually moves the money. A treasury, an enforced delay before action, and delegates who carry a public record.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <head>
        <link rel="preconnect" href="https://fonts.googleapis.com" />
        <link rel="preconnect" href="https://fonts.gstatic.com" crossOrigin="" />
        <link rel="stylesheet" href={FONTS} />
      </head>
      <body>
        <WalletProvider>
        <header className="masthead">
          <div className="masthead__inner">
            <Link href="/" className="masthead__mark">
              <b>RatifyDAO</b>
            </Link>
            <nav aria-label="Sections">
              <Link href="/communities">Communities</Link>
              <Link href="/operations">Operations</Link>
            </nav>
          </div>
        </header>

        <main className="page">{children}</main>

        <footer className="colophon">
          <div className="page">
            <p style={{ margin: 0 }}>
              RatifyDAO is a record of decisions about other people&rsquo;s money. Every
              figure on these pages is read from an index of contract events on
              Stellar, and every one of them links to the ledger entry it came
              from.
            </p>
          </div>
        </footer>
        </WalletProvider>
      </body>
    </html>
  );
}
