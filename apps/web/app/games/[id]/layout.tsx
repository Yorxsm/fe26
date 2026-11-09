import type { Metadata } from "next";
import { pageMetadata } from "@/lib/seo";

const COPY: Record<string, [string, string]> = {
  "heist-night": ["Vault", "Watch Vault matches start, play, end, and show testnet settlement state."],
  "trading-cup": ["Trading Cup", "Trading Cup is coming soon. This page does not invent seats."],
};

export async function generateMetadata({ params }: { params: Promise<{ id: string }> }): Promise<Metadata> {
  const { id } = await params;
  const [title, description] = COPY[id] ?? ["Game", "A fe2VI arena game."];
  return pageMetadata(title, description, `/games/${id}`);
}

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
