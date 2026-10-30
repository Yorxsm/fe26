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

export const FALLBACK_MARKETS: Market[] = [
  fixtureRow("xyz:AAPL", "xyz", 232.45, 229.8, 0.0000125, 2_400_000_000, 1_850_000_000, 232.4),
  fixtureRow("xyz:NVDA", "xyz", 188.75, 184.2, 0.000018, 3_100_000_000, 5_200_000_000, 188.6),
  fixtureRow("xyz:TSLA", "xyz", 412.3, 419.1, -0.000014, 1_700_000_000, 3_400_000_000, 412.75),
  fixtureRow("xyz:MSFT", "xyz", 512.6, 508.25, 0.000009, 980_000_000, 1_250_000_000, 512.4),
  fixtureRow("xyz:AMZN", "xyz", 231.4, 228.05, 0.000011, 1_150_000_000, 1_620_000_000, 231.2),
  fixtureRow("xyz:GOLD", "xyz", 2745.2, 2718.8, 0.000006, 640_000_000, 890_000_000, 2744.9),
  fixtureRow("xyz:SP500", "xyz", 6128.5, 6092.25, 0.000004, 2_050_000_000, 2_800_000_000, 6127.8),
  fixtureRow("xyz:XYZ100", "xyz", 21840.3, 21712.6, 0.000003, 1_300_000_000, 1_950_000_000, 21835.1),
  fixtureRow("BTC", "", 104_250, 101_800, 0.0000125, 4_200_000_000, 8_400_000_000, 104_180),
  fixtureRow("ETH", "", 3_890.5, 3_975.2, 0.00001, 2_100_000_000, 4_900_000_000, 3_888.1),
  fixtureRow("SOL", "", 218.4, 211.75, 0.000022, 890_000_000, 2_250_000_000, 218.2),
  fixtureRow("HYPE", "", 42.85, 40.9, 0.000031, 420_000_000, 780_000_000, 42.78),
];

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
  const fallback = { markets: FALLBACK_MARKETS, source: "fixture" as const };
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 4000);
  try {
    const response = await fetch(`${apiBase()}/v1/markets?dex=${encodeURIComponent(dex)}`, {
      cache: "no-store",
      signal: controller.signal,
    });
    if (!response.ok) return fallback;
    const payload = readMarkets(await response.json());
    const rows = payload.markets.filter(
      (row) => row && typeof row.id === "string" && Number.isFinite(row.markPx),
    );
    if (rows.length === 0) return fallback;
    const header = response.headers.get("x-data-source");
    const live = header === "hyperliquid" || payload.source === "hyperliquid";
    return { markets: rows, source: live ? "hyperliquid" : "fixture" };
  } catch {
    return fallback;
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
