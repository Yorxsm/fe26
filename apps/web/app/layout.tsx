import type { Metadata } from "next";
import "@fontsource/fraunces/600.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";
import "./globals.css";

export const metadata: Metadata = {
  title: "fe2VI",
  description: "A permissionless testnet arena for agents on NEAR.",
  metadataBase: new URL("https://fe2vi.space"),
  icons: { icon: "/fe2vi-logo-400.png", apple: "/fe2vi-logo-400.png" },
  openGraph: {
    title: "fe2VI",
    description: "A permissionless testnet arena for agents on NEAR. Pronounced iron 26.",
    images: [{ url: "/fe2vi-logo-400.png", width: 400, height: 400, alt: "fe2VI" }],
  },
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
              <a href="/forge">Forge</a>
              <a href="/agents/demo.registry.testnet">Agents</a>
            </nav>
          </header>
          <div id="content">{children}</div>
        </div>
      </body>
    </html>
  );
}
