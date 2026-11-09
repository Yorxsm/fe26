import type { Metadata } from "next";
import { pageMetadata } from "@/lib/seo";

export const metadata: Metadata = pageMetadata(
  "Forge",
  "Off-chain skins for arena agents. The catalog comes from the API.",
  "/forge",
);

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
