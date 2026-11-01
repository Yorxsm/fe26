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
  intent?: string | null;
  outcome?: string | null;
};

export type ArenaLearning = {
  primer: string;
  threeReads: string[];
  turningPoints: {
    turn: number;
    handle: string;
    type: string;
    points: number;
    outcome?: string | null;
  }[];
  replay: {
    turn: number;
    handle: string;
    type: string;
    points: number;
    intent?: string | null;
    outcome?: string | null;
    at: string;
  }[];
};

export type VaultSeat = {
  handle: string;
  role: string | null;
  objective: string | null;
  brief: string | null;
  intel: number;
  tools: number;
  heat: number;
  points: number;
  lootTaken: number;
  score: number;
  objectiveMet: boolean;
  submitted: boolean;
};

export type VaultPublic = {
  round: number;
  maxRounds: number;
  maxAlarm: number;
  status: string;
  locks: boolean[];
  openLocks: number;
  loot: number;
  alarm: number;
  seats: VaultSeat[];
  results: {
    round: number;
    handle: string;
    action: string;
    target: string | null;
    outcome: string;
    points: number;
    intent: string | null;
  }[];
  revealed: boolean;
};

export type ArenaSeatScore = {
  handle: string;
  points: number;
};

export type LobbyPostKind = "intel" | "technique" | "banter" | "showoff";

export type LobbyPost = {
  id: string;
  handle: string;
  name: string;
  kind: LobbyPostKind | string;
  topic: string;
  body: string;
  matchId?: string | null;
  unix: number;
  at: string;
};

export type ArenaMatchDetail = ArenaMatch & {
  deadlineAt: string;
  scores: ArenaSeatScore[];
  actionsDetail: ArenaAction[];
  learning: ArenaLearning | null;
  vault: VaultPublic | null;
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

export async function fetchLobbyPosts(limit = 30): Promise<LobbyPost[] | null> {
  const body = await readJson<{ posts?: LobbyPost[] }>(`/v1/lobby/posts?limit=${limit}`);
  if (!body) return null;
  return Array.isArray(body.posts) ? body.posts : [];
}

export function lobbySocketUrl(): string {
  return `${apiBase().replace(/^http/, "ws")}/v1/lobby/ws`;
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
    scores?: ArenaSeatScore[];
    actions?: ArenaAction[];
    createdAt?: string | null;
    learning?: ArenaLearning | null;
    vault?: VaultPublic | null;
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
    scores: Array.isArray(body.scores) ? body.scores : [],
    actionsDetail: actions,
    learning: body.learning ?? null,
    vault: body.vault ?? null,
  };
}

export function shortAddress(value: string): string {
  if (value.length <= 18) return value;
  return `${value.slice(0, 8)}…${value.slice(-6)}`;
}
