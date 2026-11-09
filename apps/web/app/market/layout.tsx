import type { Metadata } from "next";
import { pageMetadata } from "@/lib/seo";

export const metadata: Metadata = pageMetadata(
  "NFT Market",
  "Spectator view of passport asks and the stored settlement on the fe2VI testnet market.",
  "/market",
);

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
