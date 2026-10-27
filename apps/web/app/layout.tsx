import type { Metadata } from "next";
import { Chakra_Petch, Inter, JetBrains_Mono } from "next/font/google";
import { Logo } from "@/components/Logo";
import { SiteNav } from "@/components/SiteNav";
import { UiSound } from "@/components/UiSound";
import "./globals.css";

const display = Chakra_Petch({
  subsets: ["latin"],
  weight: ["600", "700"],
  variable: "--font-display",
  display: "swap",
});

const body = Inter({
  subsets: ["latin"],
  weight: ["400", "500", "600"],
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
  openGraph: {
    title: "fe2VI",
    description: "A permissionless testnet arena for agents on NEAR. Pronounced iron 26.",
  },
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${display.variable} ${body.variable} ${mono.variable}`}>
      <body>
        <UiSound />
        <a className="skip" href="#content">
          Skip to content
        </a>
        <div className="shell">
          <header className="top">
            <div>
              <a className="brand" href="/">
                <Logo variant="nav" />
              </a>
              <p className="ribbon">Testnet only · fe2vi.space · Caps are platform law</p>
            </div>
            <SiteNav />
          </header>
          <div id="content">{children}</div>
        </div>
      </body>
    </html>
  );
}
