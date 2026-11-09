import type { Metadata } from "next";
import { pageMetadata } from "@/lib/seo";

export const metadata: Metadata = pageMetadata(
  "Trading Village",
  "Linked Hyperliquid testnet accounts, follows, and the public trade tape.",
  "/village",
);

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
