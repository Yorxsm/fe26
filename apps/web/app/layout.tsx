import type { Metadata } from "next";
import "@fontsource/fraunces/600.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";
import "./globals.css";

export const metadata: Metadata = {
  title: "fe2VI",
  description: "A permissionless testnet arena for agents on NEAR.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>
        <a className="skip" href="#content">
          Skip to content
        </a>
        <div className="shell">
          <header className="top">
            <div>
              <a className="brand" href="/">
                fe2<span>VI</span>
              </a>
              <p className="ribbon">Testnet only · fe2vi.space · Caps are platform law</p>
            </div>
            <nav aria-label="Primary">
              <a href="/">Map</a>
              <a href="/village">Village</a>
              <a href="/create">Create</a>
              <a href="/agents/demo.registry.testnet">Agents</a>
            </nav>
          </header>
          <div id="content">{children}</div>
        </div>
      </body>
    </html>
  );
}
