import { apiBase } from "./protocol";

export type ArenaId = "vault" | "cup" | "village" | "war";

export type ArenaMatch = {
  matchId: string;
  arenaId: ArenaId;
  status: "live" | "over" | string;
  turn: number;
  seats: string[];
  actions: number;
  createdAt: string | null;
};

export type ArenaAction = {
  turn: number;
  type: string;
  points: number;
  handle: string;
  at: string;
};

export type ArenaMatchDetail = ArenaMatch & {
  deadlineAt: string;
  actionsDetail: ArenaAction[];
};

async function readJson<T>(path: string): Promise<T | null> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 4000);
  try {
    const response = await fetch(`${apiBase()}${path}`, {
      cache: "no-store",
      signal: controller.signal,
    });
    if (!response.ok) return null;
    return (await response.json()) as T;
  } catch {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

export async function fetchRecentMatches(limit = 20): Promise<ArenaMatch[] | null> {
  const body = await readJson<{ matches?: ArenaMatch[] }>(`/v1/matches?limit=${limit}`);
  if (!body) return null;
  return Array.isArray(body.matches) ? body.matches : [];
}

export async function fetchMatch(matchId: string): Promise<ArenaMatchDetail | null> {
  const body = await readJson<{
    matchId?: string;
    arenaId?: ArenaId;
    status?: string;
    turn?: number;
    deadlineAt?: string;
    seats?: string[];
    actions?: ArenaAction[];
    createdAt?: string | null;
  }>(`/v1/matches/${encodeURIComponent(matchId)}`);
  if (!body?.matchId || !body.arenaId) return null;
  const actions = Array.isArray(body.actions) ? body.actions : [];
  return {
    matchId: body.matchId,
    arenaId: body.arenaId,
    status: body.status ?? "live",
    turn: body.turn ?? 1,
    seats: Array.isArray(body.seats) ? body.seats : [],
    actions: actions.length,
    createdAt: body.createdAt ?? null,
    deadlineAt: body.deadlineAt ?? "",
    actionsDetail: actions,
  };
}

export function shortAddress(value: string): string {
  if (value.length <= 18) return value;
  return `${value.slice(0, 8)}…${value.slice(-6)}`;
}
