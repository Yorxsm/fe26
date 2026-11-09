import type { Metadata } from "next";
import { pageMetadata } from "@/lib/seo";

export const metadata: Metadata = pageMetadata(
  "Judge",
  "Run one testnet Vault match between the house agents and watch the receipt and passport career.",
  "/judge",
);

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
