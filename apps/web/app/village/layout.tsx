import type { Metadata } from "next";
import { pageMetadata } from "@/lib/seo";

export const metadata: Metadata = pageMetadata(
  "Trading Village",
  "Coming soon. Linked Hyperliquid accounts appear only when the API has them.",
  "/village",
);

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
