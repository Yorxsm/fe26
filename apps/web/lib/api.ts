import { fixtureAgent, fixtureEvents, fixtureGame } from "./fixtures";
import { apiBase, type AgentCard, type FeedEvent, type GameCard } from "./protocol";

export async function loadGames(): Promise<{ games: GameCard[]; source: "relay" | "fixture" }> {
  const base = apiBase();
  if (base) {
    try {
      const response = await fetch(`${base}/v1/games`, { cache: "no-store" });
      if (response.ok) {
        const games = (await response.json()) as GameCard[];
        if (games.length > 0) return { games, source: "relay" };
      }
    } catch {
      // Fall through to the local fixture so the lobby still renders.
    }
  }
  return { games: [fixtureGame], source: "fixture" };
}

export async function loadGame(id: string): Promise<{ game: GameCard | null; source: "relay" | "fixture" }> {
  const base = apiBase();
  if (base) {
    try {
      const response = await fetch(`${base}/v1/games/${encodeURIComponent(id)}`, { cache: "no-store" });
      if (response.ok) return { game: (await response.json()) as GameCard, source: "relay" };
      if (response.status === 404) return { game: null, source: "relay" };
    } catch {
      // Fixture below.
    }
  }
  if (id === fixtureGame.id) return { game: fixtureGame, source: "fixture" };
  return { game: null, source: "fixture" };
}

export async function loadAgent(id: string): Promise<{ agent: AgentCard | null; source: "relay" | "fixture" }> {
  const base = apiBase();
  if (base) {
    try {
      const response = await fetch(`${base}/v1/agents/${encodeURIComponent(id)}`, { cache: "no-store" });
      if (response.ok) return { agent: (await response.json()) as AgentCard, source: "relay" };
      if (response.status === 404) return { agent: null, source: "relay" };
    } catch {
      // Fixture below.
    }
  }
  return { agent: fixtureAgent(id) ?? null, source: "fixture" };
}

export function fixtureFeed(id: string): FeedEvent[] {
  return fixtureEvents.filter((event) => event.game_id === id);
}
