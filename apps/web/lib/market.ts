import { apiBase } from "./protocol";
import {
  marketFromResponse,
  type MarketListing,
  type MarketPayload,
  type MarketSale,
  type MarketStats,
} from "./marketView";

export {
  FALLBACK_LISTINGS,
  FALLBACK_SALES,
  FALLBACK_STATS,
  marketFromResponse,
  type MarketListing,
  type MarketPayload,
  type MarketSale,
  type MarketStats,
} from "./marketView";

export async function fetchMarketData(): Promise<{
  listings: MarketListing[];
  sales: MarketSale[];
  stats: MarketStats;
  contract: string;
  network: string;
  blockers: string[];
  lastSettlement: { buyer?: string; seller?: string; priceNear?: string; note?: string } | null;
  source: "chain" | "not-read" | "unavailable";
}> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 35000);
  try {
    const response = await fetch(`${apiBase()}/v1/market/listings`, {
      cache: "no-store",
      signal: controller.signal,
    });
    if (!response.ok) return marketFromResponse(false, null);
    const body = (await response.json()) as MarketPayload;
    if (Array.isArray(body.sales)) return marketFromResponse(true, body);
    try {
      const salesResponse = await fetch(`${apiBase()}/v1/market/sales`, {
        cache: "no-store",
        signal: controller.signal,
      });
      if (salesResponse.ok) {
        const salesBody = (await salesResponse.json()) as { sales?: MarketSale[] };
        if (Array.isArray(salesBody.sales)) body.sales = salesBody.sales;
      }
    } catch {
      body.sales = [];
    }
    return marketFromResponse(true, body);
  } catch {
    return marketFromResponse(false, null);
  } finally {
    clearTimeout(timer);
  }
}
