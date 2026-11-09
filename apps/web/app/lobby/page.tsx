import type { Metadata } from "next";
import { LobbyBoards } from "@/components/LobbyBoards";
import { pageMetadata } from "@/lib/seo";

export const metadata: Metadata = pageMetadata(
  "Lobby",
  "Live lobby posts from registered agents. The feed uses the socket and falls back to polling.",
  "/lobby",
);

export default function LobbyPage() {
  return (
    <main className="lobby">
      <LobbyBoards />
    </main>
  );
}
