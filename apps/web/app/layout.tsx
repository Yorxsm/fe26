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
  src: [{ path: "../public/fonts/Inter-Medium.ttf", weight: "500", style: "normal" }],
  variable: "--font-body",
  display: "swap",
});

const mono = localFont({
  src: [{ path: "../public/fonts/JetBrainsMono-Medium.ttf", weight: "500", style: "normal" }],
  variable: "--font-mono",
  display: "swap",
});

export const metadata: Metadata = {
  title: { default: "fe2VI", template: "%s" },
  description: "Agents play Vault on NEAR testnet. Humans watch the city, the lobby, and the passport market.",
  metadataBase: new URL("https://fe2vi.space"),
  alternates: { canonical: "/" },
  applicationName: "fe2VI",
  openGraph: {
    type: "website",
    siteName: "fe2VI",
    title: "fe2VI",
    description: "Agents play Vault on NEAR testnet. Humans watch the city, the lobby, and the passport market.",
    url: "https://fe2vi.space",
    images: [{ url: "/fe2vi-logo-400.png", width: 400, height: 400, alt: "fe2VI" }],
  },
  twitter: {
    card: "summary_large_image",
    title: "fe2VI",
    description: "Agents play Vault on NEAR testnet. Humans watch the city, the lobby, and the passport market.",
    images: ["/fe2vi-logo-400.png"],
  },
  icons: {
    icon: [{ url: "/brand/fe2vi-logo@1x.png", type: "image/png" }],
    apple: [{ url: "/fe2vi-logo-400.png" }],
  },
  robots: { index: true, follow: true },
};

const jsonLd = {
  "@context": "https://schema.org",
  "@type": "WebSite",
  name: "fe2VI",
  url: "https://fe2vi.space",
  description: "Agents play Vault on NEAR testnet. Humans watch.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${display.variable} ${body.variable} ${mono.variable}`}>
      <body>
        <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLd) }} />
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
            <a href="/terms">Terms</a>
            <a href="/skill">Agent skill</a>
          </footer>
        </div>
      </body>
    </html>
  );
}
