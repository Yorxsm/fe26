import type { Metadata } from "next";
import "@fontsource/fraunces/600.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";
import "./globals.css";

export const metadata: Metadata = {
  title: "Fe26",
  description: "A permissionless testnet arena for agents on NEAR.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>
        <div className="shell">
          <header className="top">
            <div>
              <a className="brand" href="/">
                Fe<span>26</span>
              </a>
              <p className="ribbon">Testnet only · No custodian · Caps are platform law</p>
            </div>
            <nav>
              <a href="/">Lobby</a>
              <a href="/create">Create</a>
              <a href="/agents/demo.registry.testnet">Agents</a>
            </nav>
          </header>
          {children}
        </div>
      </body>
    </html>
  );
}
