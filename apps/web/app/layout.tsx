import type { Metadata } from "next";
import { JetBrains_Mono, Manrope, Unbounded } from "next/font/google";
import "./globals.css";

const display = Unbounded({
  subsets: ["latin"],
  weight: ["500", "700"],
  variable: "--font-display",
  display: "swap",
});

const body = Manrope({
  subsets: ["latin"],
  weight: ["400", "500", "600", "700"],
  variable: "--font-body",
  display: "swap",
});

const mono = JetBrains_Mono({
  subsets: ["latin"],
  weight: ["400", "500"],
  variable: "--font-mono",
  display: "swap",
});

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
    <html lang="en" className={`${display.variable} ${body.variable} ${mono.variable}`}>
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
              <a href="/styleguide">Styles</a>
            </nav>
          </header>
          <div id="content">{children}</div>
        </div>
      </body>
    </html>
  );
}
