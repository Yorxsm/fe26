import type { PublicRow } from "./api";
import { apiBase } from "./protocol";

export type HallRoom = {
  room: string;
  leader: PublicRow | null;
};

export type HallOfFame = {
  window: string;
  now: number;
  totalAgents: number;
  champions: PublicRow[];
  rooms: HallRoom[];
  season: PublicRow[];
};

export async function fetchHallOfFame(): Promise<HallOfFame | null> {
  try {
    const response = await fetch(`${apiBase()}/v1/hall-of-fame`, { cache: "no-store" });
    if (!response.ok) return null;
    const body = (await response.json()) as Partial<HallOfFame>;
    if (!Array.isArray(body.champions) || !Array.isArray(body.rooms) || !Array.isArray(body.season)) {
      return null;
    }
    return {
      window: body.window ?? "all",
      now: body.now ?? 0,
      totalAgents: body.totalAgents ?? body.champions.length,
      champions: body.champions,
      rooms: body.rooms,
      season: body.season,
    };
  } catch {
    return null;
  }
}
