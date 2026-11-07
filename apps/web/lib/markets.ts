import { apiBase } from "./protocol";

export type MarketKind = "crypto" | "stock" | "etf" | "index" | "commodity" | "fx" | "other";

export type Market = {
  id: string;
  symbol: string;
  dex: string;
  kind: MarketKind;
  markPx: number;
  prevDayPx: number;
  changePx: number;
  changePct: number;
  funding: number;
  openInterest: number;
  volume24h: number;
  oraclePx: number;
};

export type Candle = {
  t: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
};

export type MarketSource = "hyperliquid" | "fixture";

function fixtureRow(
  id: string,
  dex: string,
  markPx: number,
  prevDayPx: number,
  funding: number,
  openInterest: number,
  volume24h: number,
  oraclePx: number,
): Market {
  const symbol = id.includes(":") ? id.slice(id.lastIndexOf(":") + 1) : id;
  const changePx = markPx - prevDayPx;
  const changePct = prevDayPx === 0 ? 0 : (changePx / prevDayPx) * 100;
  const kind: MarketKind =
    symbol === "BTC" || symbol === "ETH" || symbol === "SOL" || symbol === "HYPE"
      ? "crypto"
      : id === "xyz:GOLD"
        ? "commodity"
        : id === "xyz:SP500" || id === "xyz:XYZ100"
          ? "index"
          : dex === "xyz"
            ? "stock"
            : "other";
  return {
    id,
    symbol,
    dex,
    kind,
    markPx,
    prevDayPx,
    changePx,
    changePct,
    funding,
    openInterest,
    volume24h,
    oraclePx,
  };
}

function readMarkets(payload: unknown): { markets: Market[]; source?: MarketSource } {
  if (Array.isArray(payload)) {
    return { markets: payload as Market[] };
  }
  if (payload && typeof payload === "object") {
    const wrapper = payload as { markets?: unknown; source?: unknown };
    const markets = Array.isArray(wrapper.markets) ? (wrapper.markets as Market[]) : [];
    const source = wrapper.source === "hyperliquid" || wrapper.source === "fixture" ? wrapper.source : undefined;
    return { markets, source };
  }
  return { markets: [] };
}

export async function fetchMarkets(
  dex = "xyz",
): Promise<{ markets: Market[]; source: MarketSource }> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 4000);
  try {
    const response = await fetch(`${apiBase()}/v1/markets?dex=${encodeURIComponent(dex)}`, {
      cache: "no-store",
      signal: controller.signal,
    });
    if (!response.ok) return { markets: [], source: "fixture" };
    const payload = readMarkets(await response.json());
    const rows = payload.markets.filter(
      (row) => row && typeof row.id === "string" && Number.isFinite(row.markPx),
    );
    const header = response.headers.get("x-data-source");
    const live = header === "hyperliquid" || payload.source === "hyperliquid";
    return { markets: rows, source: live ? "hyperliquid" : "fixture" };
  } catch {
    return { markets: [], source: "fixture" };
  } finally {
    clearTimeout(timer);
  }
}

export function formatPrice(value: number): string {
  if (!Number.isFinite(value)) return "—";
  const abs = Math.abs(value);
  if (abs >= 1) return value.toFixed(2);
  return value.toPrecision(4);
}

export function formatPct(value: number): string {
  if (!Number.isFinite(value)) return "—";
  const sign = value > 0 ? "+" : "";
  return `${sign}${value.toFixed(2)}%`;
}
