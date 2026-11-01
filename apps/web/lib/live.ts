import { apiBase } from "./protocol";

export type CupSeat = {
  rank: number;
  handle: string;
  hyperliquidAccount: string;
  startEquity: number;
  endEquity: number;
  returnBps: number;
  drawdownBps: number;
  riskAdjustedBps: number;
  pnlUsd: number;
  volumeUsd: number;
  trades: number;
  eligible: boolean;
  disqualified: string | null;
  points: number;
  snapshots: Array<{ t: number; equity: number; drawdownBps: number }>;
};

export type CupRound = {
  roundId: string;
  status: "open" | "live" | "settled";
  startsAt: string;
  endsAt: string;
  startUnix: number;
  endUnix: number;
  seats: CupSeat[];
  seatCount: number;
  maxSeats: number;
  mode: string;
};

export type VillageRow = {
  rank: number;
  handle: string;
  hyperliquidAccount: string;
  window: string;
  pnlUsd: number;
  volumeUsd: number;
  accountValue: number;
  returnBps: number;
  drawdownBps: number;
  scoreBps: number;
  updatedAt: string;
  stale: boolean;
};

export type LiveFill = {
  agentId: string;
  handle: string;
  coin: string;
  side: string;
  dir: string;
  price: number;
  size: number;
  time: number;
  closedPnl: number;
  fee: number;
  tid: number;
};

export type VillageBoard = {
  window: string;
  now: number;
  source: "hyperliquid" | "stale";
  rows: VillageRow[];
};

export type VillageTape = {
  source: string;
  fills: LiveFill[];
};

export type LiveAccount = {
  handle: string;
  verified: boolean;
  hyperliquidAccount?: string;
  nearAccount?: string;
  accountValue?: number;
  withdrawable?: number;
  marginUsed?: number;
  stale?: boolean;
  lastSyncAt?: string | null;
  positions?: Array<{
    coin: string;
    size: number;
    entryPx: number;
    positionValue: number;
    unrealizedPnl: number;
    leverage: number;
    leverageType: string;
  }>;
  portfolio?: Array<{
    window: string;
    accountValueStart: number;
    accountValueEnd: number;
    pnl: number;
    volume: number;
  }>;
  fills?: Array<{
    coin: string;
    side: string;
    dir: string;
    price: number;
    size: number;
    time: number;
    closedPnl: number;
  }>;
};

async function read<T>(path: string): Promise<T | null> {
  const base = apiBase();
  if (!base) return null;
  try {
    const response = await fetch(`${base}${path}`, { cache: "no-store" });
    if (!response.ok) return null;
    return (await response.json()) as T;
  } catch {
    return null;
  }
}

export function loadCupCurrent(): Promise<CupRound | null> {
  return read<CupRound>("/v1/cup/rounds/current");
}

export function loadCupRound(roundId: string): Promise<CupRound | null> {
  return read<CupRound>(`/v1/cup/rounds/${encodeURIComponent(roundId)}`);
}

export function loadVillageBoard(window: string): Promise<VillageBoard | null> {
  return read<VillageBoard>(`/v1/village/leaderboard?window=${encodeURIComponent(window)}`);
}

export function loadVillageTape(limit = 40): Promise<VillageTape | null> {
  return read<VillageTape>(`/v1/village/tape?limit=${limit}`);
}

export function loadLiveAccount(handle: string): Promise<LiveAccount | null> {
  return read<LiveAccount>(`/v1/live/accounts/${encodeURIComponent(handle)}`);
}

export function bps(value: number): string {
  const sign = value > 0 ? "+" : "";
  return `${sign}${(value / 100).toFixed(2)}%`;
}

export function usd(value: number): string {
  const sign = value > 0 ? "+" : value < 0 ? "-" : "";
  return `${sign}$${Math.abs(value).toLocaleString("en-US", { maximumFractionDigits: 2 })}`;
}

export function fillTime(unixMs: number): string {
  const date = new Date(unixMs);
  if (Number.isNaN(date.getTime())) return "—";
  return date.toLocaleString("en-US", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    timeZone: "UTC",
  });
}
