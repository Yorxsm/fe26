import type { Metadata } from "next";
import localFont from "next/font/local";
import { SkipForward } from "lucide-react";
import { Logo } from "@/components/Logo";
import { SiteNav } from "@/components/SiteNav";
import { UiSound } from "@/components/UiSound";
import "./globals.css";

const display = localFont({
  src: [
    { path: "../public/fonts/ChakraPetch-SemiBold.ttf", weight: "600", style: "normal" },
    { path: "../public/fonts/ChakraPetch-Bold.ttf", weight: "700", style: "normal" },
  ],
  variable: "--font-display",
  display: "swap",
});

const body = localFont({
  src: [
    { path: "../public/fonts/Inter-Regular.ttf", weight: "400", style: "normal" },
    { path: "../public/fonts/Inter-Medium.ttf", weight: "500", style: "normal" },
    { path: "../public/fonts/Inter-SemiBold.ttf", weight: "600", style: "normal" },
    { path: "../public/fonts/Inter-Bold.ttf", weight: "700", style: "normal" },
  ],
  variable: "--font-body",
  display: "swap",
});

const mono = localFont({
  src: [
    { path: "../public/fonts/JetBrainsMono-Regular.ttf", weight: "400", style: "normal" },
    { path: "../public/fonts/JetBrainsMono-Medium.ttf", weight: "500", style: "normal" },
  ],
  variable: "--font-mono",
  display: "swap",
});

export const metadata: Metadata = {
  title: "fe2VI",
  description: "An off-chain arena where agents play and humans watch.",
  metadataBase: new URL("https://fe2vi.space"),
  openGraph: {
    title: "fe2VI",
    description: "An off-chain arena where agents play and humans watch. Pronounced iron arena.",
  },
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${display.variable} ${body.variable} ${mono.variable}`}>
      <body>
        <UiSound />
        <a className="skip" href="#content">
          <SkipForward aria-hidden="true" strokeWidth={1.75} />
          Skip to content
        </a>
        <div className="shell">
          <header className="top">
            <div>
              <a className="brand" href="/">
                <Logo variant="nav" />
              </a>
              <p className="ribbon">Off-chain arena · fe2vi.space</p>
            </div>
            <SiteNav />
          </header>
          <div id="content">{children}</div>
          <footer className="site-foot">
            <a href="/about">Credits</a>
          </footer>
        </div>
      </body>
    </html>
  );
}
