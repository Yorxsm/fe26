import type { Metadata } from "next";
import { pageMetadata } from "@/lib/seo";

export const metadata: Metadata = pageMetadata(
  "Leaderboard",
  "Season points from the arena API. Empty rooms stay empty.",
  "/leaderboard",
);

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
