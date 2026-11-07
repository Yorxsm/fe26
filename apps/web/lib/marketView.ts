type SkinTraits = {
  paint: number;
  screen: number;
  eyes: number;
  headwear: number;
  face_item: number;
  back_item: number;
  emblem: number;
  trail: number;
  role: number;
};

export type MarketListing = {
  tokenId: string;
  name: string;
  seller: string;
  priceNear: string;
  priceYocto: string;
  rarity?: "Common" | "Rare" | "Mythic";
  description: string;
  traits?: SkinTraits;
  trophies?: Array<{
    name: string;
    description: string;
    icon: string;
    season: number;
  }>;
  owner?: string;
  approvalId?: number;
  career?: {
    xp: number;
    wins: number;
    matches: number;
    rank: number;
    bestRank: number;
  };
};

export type MarketSale = {
  id: string;
  tokenId: string;
  buyer: string;
  seller: string;
  priceNear: string;
  timestamp: string;
  rarity: "Common" | "Rare" | "Mythic";
};

export type MarketStats = {
  floorPriceNear: string | null;
  totalVolumeNear: string | null;
  listedCount: number;
  totalSalesCount: number | null;
  activeSpectators: number | null;
};

export type MarketPayload = {
  listings?: MarketListing[];
  sales?: MarketSale[];
  stats?: MarketStats;
  contract?: string;
  network?: string;
  source?: string;
  blockers?: string[];
  lastSettlement?: {
    buyer?: string;
    seller?: string;
    priceNear?: string;
    note?: string;
  } | null;
};

const EMPTY_STATS: MarketStats = {
  floorPriceNear: null,
  totalVolumeNear: null,
  listedCount: 0,
  totalSalesCount: null,
  activeSpectators: null,
};

/** A successful body is shown as returned. A failed read stays empty and is not replaced with samples. */
export function marketFromResponse(ok: boolean, body: MarketPayload | null): {
  listings: MarketListing[];
  sales: MarketSale[];
  stats: MarketStats;
  contract: string;
  network: string;
  blockers: string[];
  lastSettlement: { buyer?: string; seller?: string; priceNear?: string; note?: string } | null;
  source: "chain" | "not-read" | "unavailable";
} {
  if (!ok || !body) {
    return {
      listings: [],
      sales: [],
      stats: EMPTY_STATS,
      contract: "",
      network: "",
      blockers: [],
      lastSettlement: null,
      source: "unavailable",
    };
  }
  const listings = Array.isArray(body.listings) ? body.listings : [];
  const sales = Array.isArray(body.sales) ? body.sales : [];
  const source = body.source === "not-read" ? "not-read" : "chain";
  return {
    listings,
    sales,
    stats: body.stats ?? { ...EMPTY_STATS, listedCount: listings.length },
    contract: typeof body.contract === "string" ? body.contract : "",
    network: typeof body.network === "string" ? body.network : "",
    blockers: Array.isArray(body.blockers) ? body.blockers.filter((item) => typeof item === "string") : [],
    lastSettlement: body.lastSettlement && typeof body.lastSettlement === "object" ? body.lastSettlement : null,
    source,
  };
}

