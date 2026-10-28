import { fixtureAgent, fixtureEvents, fixtureGames } from "./fixtures";
import { apiBase, type AgentCard, type FeedEvent, type GameCard } from "./protocol";
import { FIXTURE_AGENTS } from "./schedule";

export async function loadGames(): Promise<{ games: GameCard[]; source: "relay" | "fixture" }> {
  const base = apiBase();
  if (base) {
    try {
      const response = await fetch(`${base}/v1/games`, { cache: "no-store" });
      if (response.ok) {
        const games = (await response.json()) as GameCard[];
        return { games, source: "relay" };
      }
    } catch {
      // Fall through to the local fixture so the lobby still renders.
    }
  }
  return { games: fixtureGames, source: "fixture" };
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
  const fixture = fixtureGames.find((game) => game.id === id);
  return { game: fixture ?? null, source: "fixture" };
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

export type LobbyHeistRow = { agent: string; name: string; points: number };

export type LobbyRelay = {
  now: number;
  live: boolean;
  seasonStart: number;
  seasonEnd: number;
  slot: { kind: string; start: number; end: number };
  heist: Record<"today" | "week" | "season", LobbyHeistRow[]>;
  quotes: { symbol: string; kind: string; markMicros: number; fundingBps: number; pnlMicros: number }[];
  challenges: { id: string; category: string; title: string; points: number }[];
  events: {
    id: string;
    partner: string;
    exampleOnly: boolean;
    room: string;
    prize: string;
    start: number;
    end: number;
  }[];
};

export type HomeRelay = {
  now: number;
  agents: number;
  agentsFrom: "relay" | "fixture";
  events: { exampleOnly: boolean; start: number; end: number }[];
};

export async function loadLobby(): Promise<
  { ok: true; relay: LobbyRelay } | { ok: false; reason: "unset" | "unreachable" }
> {
  const base = apiBase();
  if (!base) return { ok: false, reason: "unset" };
  try {
    const [schedule, today, week, season, venues, challenges, events] = await Promise.all([
      fetch(`${base}/v1/schedule`, { cache: "no-store" }).then(readJson),
      fetch(`${base}/v1/heist/leaderboard?window=today`, { cache: "no-store" }).then(readJson),
      fetch(`${base}/v1/heist/leaderboard?window=week`, { cache: "no-store" }).then(readJson),
      fetch(`${base}/v1/heist/leaderboard?window=season`, { cache: "no-store" }).then(readJson),
      fetch(`${base}/v1/venues`, { cache: "no-store" }).then(readJson),
      fetch(`${base}/v1/challenges`, { cache: "no-store" }).then(readJson),
      fetch(`${base}/v1/events`, { cache: "no-store" }).then(readJson),
    ]);
    const relay = parseLobby(schedule, today, week, season, venues, challenges, events);
    return relay ? { ok: true, relay } : { ok: false, reason: "unreachable" };
  } catch {
    return { ok: false, reason: "unreachable" };
  }
}

export async function loadHome(): Promise<
  { ok: true; relay: HomeRelay } | { ok: false; reason: "unset" | "unreachable" }
> {
  const base = apiBase();
  if (!base) return { ok: false, reason: "unset" };
  try {
    const [statsRes, scheduleRes, eventsRes] = await Promise.all([
      fetch(`${base}/v1/stats`, { cache: "no-store" }),
      fetch(`${base}/v1/schedule`, { cache: "no-store" }),
      fetch(`${base}/v1/events`, { cache: "no-store" }),
    ]);
    if (!scheduleRes.ok || !eventsRes.ok) return { ok: false, reason: "unreachable" };
    const schedule = (await scheduleRes.json()) as unknown;
    const events = (await eventsRes.json()) as unknown;
    const stats = statsRes.ok ? ((await statsRes.json()) as unknown) : null;
    const relay = parseHome(stats, schedule, events);
    if (relay) return { ok: true, relay };
    const localAgents = parseHome({ network: "mock", agents: FIXTURE_AGENTS.length }, schedule, events);
    if (!localAgents) return { ok: false, reason: "unreachable" };
    return { ok: true, relay: { ...localAgents, agentsFrom: "fixture" } };
  } catch {
    return { ok: false, reason: "unreachable" };
  }
}

async function readJson(response: Response): Promise<unknown> {
  if (!response.ok) throw new Error(String(response.status));
  return response.json() as Promise<unknown>;
}

function record(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" ? (value as Record<string, unknown>) : null;
}

function num(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function text(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 ? value : null;
}

function heistRows(value: unknown): LobbyHeistRow[] | null {
  const body = record(value);
  if (!body || !Array.isArray(body.rows)) return null;
  const rows: LobbyHeistRow[] = [];
  for (const row of body.rows) {
    const item = record(row);
    const agent = item ? text(item.agent) : null;
    const name = item ? text(item.name) : null;
    const points = item ? num(item.points) : null;
    if (!agent || !name || points == null) return null;
    rows.push({ agent, name, points });
  }
  return rows;
}

function parseLobby(
  schedule: unknown,
  today: unknown,
  week: unknown,
  season: unknown,
  venues: unknown,
  challenges: unknown,
  events: unknown,
): LobbyRelay | null {
  const clock = record(schedule);
  const next = clock ? record(clock.next) : null;
  const now = clock ? num(clock.now) : null;
  const seasonStart = clock ? num(clock.season_start) : null;
  const seasonEnd = clock ? num(clock.season_end) : null;
  const start = next ? num(next.start_unix) : null;
  const end = next ? num(next.end_unix) : null;
  const kind = next ? text(next.kind) : null;
  if (now == null || seasonStart == null || seasonEnd == null || start == null || end == null || !kind) return null;
  const open = clock?.live;
  if (typeof open !== "boolean") return null;
  const heistToday = heistRows(today);
  const heistWeek = heistRows(week);
  const heistSeason = heistRows(season);
  if (!heistToday || !heistWeek || !heistSeason) return null;
  const tape = record(venues);
  if (!tape || !Array.isArray(tape.quotes)) return null;
  const quotes = [];
  for (const row of tape.quotes) {
    const item = record(row);
    const symbol = item ? text(item.symbol) : null;
    const quoteKind = item ? text(item.kind) : null;
    const markMicros = item ? num(item.mark_micros) : null;
    const fundingBps = item ? num(item.funding_bps) : null;
    const pnlMicros = item ? num(item.pnl_micros) : null;
    if (!symbol || !quoteKind || markMicros == null || fundingBps == null || pnlMicros == null) return null;
    quotes.push({ symbol, kind: quoteKind, markMicros, fundingBps, pnlMicros });
  }
  const board = record(challenges);
  if (!board || !Array.isArray(board.challenges)) return null;
  const parsedChallenges = [];
  for (const row of board.challenges) {
    const item = record(row);
    const id = item ? text(item.id) : null;
    const category = item ? text(item.category) : null;
    const title = item ? text(item.title) : null;
    const points = item ? num(item.points) : null;
    if (!id || !category || !title || points == null) return null;
    parsedChallenges.push({ id, category, title, points });
  }
  const hosted = record(events);
  if (!hosted || !Array.isArray(hosted.events)) return null;
  const parsedEvents = [];
  for (const row of hosted.events) {
    const item = record(row);
    const id = item ? text(item.id) : null;
    const partner = item ? text(item.partner_label) : null;
    const room = item ? text(item.room_id) : null;
    const prize = item ? text(item.prize_text) : null;
    const start = item ? num(item.start_unix) : null;
    const end = item ? num(item.end_unix) : null;
    if (!id || !partner || !room || !prize || start == null || end == null || typeof item?.example_only !== "boolean") return null;
    parsedEvents.push({ id, partner, exampleOnly: item.example_only, room, prize, start, end });
  }
  return {
    now,
    live: open,
    seasonStart,
    seasonEnd,
    slot: { kind, start, end },
    heist: { today: heistToday, week: heistWeek, season: heistSeason },
    quotes,
    challenges: parsedChallenges,
    events: parsedEvents,
  };
}

function parseHome(stats: unknown, schedule: unknown, events: unknown): HomeRelay | null {
  const clock = record(stats);
  const board = record(schedule);
  const hosted = record(events);
  const agents = clock ? num(clock.agents) : null;
  const stamp = board ? num(board.now) : null;
  if (agents == null || stamp == null || clock?.network !== "mock" || !hosted || !Array.isArray(hosted.events)) return null;
  const agentsFrom: HomeRelay["agentsFrom"] = "relay";
  const parsed = [];
  for (const row of hosted.events) {
    const item = record(row);
    if (!item || typeof item.example_only !== "boolean") return null;
    const start = num(item.start_unix);
    const end = num(item.end_unix);
    if (start == null || end == null) return null;
    parsed.push({ exampleOnly: item.example_only, start, end });
  }
  return { now: stamp, agents, agentsFrom, events: parsed };
}
