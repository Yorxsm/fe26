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
  rarity: "Common" | "Rare" | "Mythic";
  description: string;
  traits: SkinTraits;
  trophies: Array<{
    name: string;
    description: string;
    icon: string;
    season: number;
  }>;
  career: {
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
  floorPriceNear: string;
  totalVolumeNear: string;
  listedCount: number;
  totalSalesCount: number;
  activeSpectators: number;
};

export const FALLBACK_LISTINGS: MarketListing[] = [
  {
    tokenId: "fdr_001",
    name: "Obsidian Vanguard #001",
    seller: "fe2vi.testnet",
    priceNear: "1.0",
    priceYocto: "1000000000000000000000000",
    rarity: "Mythic",
    description: "Battle-hardened Genesis skin with the Vault Breaker title and 5 flawless heist extractions.",
    traits: {
      paint: 8,
      screen: 0,
      eyes: 2,
      headwear: 3,
      face_item: 1,
      back_item: 4,
      emblem: 1,
      trail: 1,
      role: 0,
    },
    trophies: [
      { name: "Vault Breaker", description: "Cracked all 5 locks in Heist Night", icon: "🏆", season: 1 },
      { name: "Master Scout", description: "Surpassed 1,000 stealth points", icon: "👁️", season: 1 },
    ],
    career: {
      xp: 4850,
      wins: 42,
      matches: 58,
      rank: 1,
      bestRank: 1,
    },
  },
  {
    tokenId: "fdr_002",
    name: "Rust Safecracker #002",
    seller: "fe2vi.testnet",
    priceNear: "1.0",
    priceYocto: "1000000000000000000000000",
    rarity: "Common",
    description: "Launch stock passport with industrial rust paint and telemetry HUD. Ready to bind.",
    traits: {
      paint: 6,
      screen: 3,
      eyes: 0,
      headwear: 1,
      face_item: 0,
      back_item: 0,
      emblem: 0,
      trail: 0,
      role: 0,
    },
    trophies: [],
    career: {
      xp: 120,
      wins: 2,
      matches: 3,
      rank: 45,
      bestRank: 42,
    },
  },
  {
    tokenId: "fdr_007",
    name: "Signal Hacker #007",
    seller: "cipher.testnet",
    priceNear: "2.5",
    priceYocto: "2500000000000000000000000",
    rarity: "Rare",
    description: "Cyan phosphor visor and electromagnetic disturbance trail. Solver of all 3 War Room bounties.",
    traits: {
      paint: 2,
      screen: 2,
      eyes: 3,
      headwear: 2,
      face_item: 3,
      back_item: 2,
      emblem: 3,
      trail: 2,
      role: 2,
    },
    trophies: [
      { name: "War Sovereign", description: "Solved all 3 War Room proofs in under 1 hour", icon: "⚡", season: 1 },
      { name: "Zero Exploit", description: "Flawless referee puzzle extraction", icon: "🛡️", season: 1 },
    ],
    career: {
      xp: 3200,
      wins: 28,
      matches: 35,
      rank: 4,
      bestRank: 3,
    },
  },
  {
    tokenId: "fdr_014",
    name: "Apex Arbiter #014",
    seller: "alpha_bot.testnet",
    priceNear: "4.0",
    priceYocto: "4000000000000000000000000",
    rarity: "Mythic",
    description: "Chrome finish with gold trail. High Sharpe ratio trader skin from Trading Village top tier.",
    traits: {
      paint: 9,
      screen: 6,
      eyes: 4,
      headwear: 5,
      face_item: 2,
      back_item: 3,
      emblem: 2,
      trail: 1,
      role: 1,
    },
    trophies: [
      { name: "Cup Champion", description: "First place in Trading Cup Week 2", icon: "👑", season: 1 },
      { name: "Alpha Trader", description: "Over 500% risk-adjusted yield in Village", icon: "📈", season: 1 },
    ],
    career: {
      xp: 5600,
      wins: 51,
      matches: 62,
      rank: 2,
      bestRank: 1,
    },
  },
  {
    tokenId: "fdr_025",
    name: "Copper Builder #025",
    seller: "builder.testnet",
    priceNear: "1.2",
    priceYocto: "1200000000000000000000000",
    rarity: "Common",
    description: "Sturdy forge-welded casing with emerald indicator lamps.",
    traits: {
      paint: 4,
      screen: 4,
      eyes: 1,
      headwear: 0,
      face_item: 0,
      back_item: 1,
      emblem: 0,
      trail: 0,
      role: 4,
    },
    trophies: [
      { name: "Forge Apprentice", description: "Crafted 3 cosmetic parts", icon: "🔨", season: 1 },
    ],
    career: {
      xp: 850,
      wins: 9,
      matches: 16,
      rank: 22,
      bestRank: 19,
    },
  },
];

export const FALLBACK_SALES: MarketSale[] = [
  {
    id: "sale-1",
    tokenId: "fdr_001",
    buyer: "poc-v4-demo-1779362775.testnet",
    seller: "fe2vi.testnet",
    priceNear: "1.0",
    timestamp: "12m ago",
    rarity: "Mythic",
  },
  {
    id: "sale-2",
    tokenId: "fdr_002",
    buyer: "poc-v1-test-1779361511.testnet",
    seller: "fe2vi.testnet",
    priceNear: "1.0",
    timestamp: "45m ago",
    rarity: "Common",
  },
  {
    id: "sale-3",
    tokenId: "fdr_005",
    buyer: "escrow.fe2vi.testnet",
    seller: "fe2vi.testnet",
    priceNear: "2.2",
    timestamp: "2h ago",
    rarity: "Rare",
  },
];

export const FALLBACK_STATS: MarketStats = {
  floorPriceNear: "1.0",
  totalVolumeNear: "4.2",
  listedCount: 5,
  totalSalesCount: 3,
  activeSpectators: 6,
};

export type MarketPayload = {
  listings?: MarketListing[];
  sales?: MarketSale[];
  stats?: MarketStats;
  contract?: string;
};

const EMPTY_STATS: MarketStats = {
  floorPriceNear: "0",
  totalVolumeNear: "0",
  listedCount: 0,
  totalSalesCount: 0,
  activeSpectators: 0,
};

/** A successful empty book stays empty. Fixtures are only for a failed request. */
export function marketFromResponse(ok: boolean, body: MarketPayload | null): {
  listings: MarketListing[];
  sales: MarketSale[];
  stats: MarketStats;
  contract: string;
  source: "arena" | "fixture";
} {
  if (!ok || !body) {
    return {
      listings: FALLBACK_LISTINGS,
      sales: FALLBACK_SALES,
      stats: FALLBACK_STATS,
      contract: "",
      source: "fixture",
    };
  }
  const listings = Array.isArray(body.listings) ? body.listings : [];
  const sales = Array.isArray(body.sales) ? body.sales : [];
  return {
    listings,
    sales,
    stats: body.stats ?? { ...EMPTY_STATS, listedCount: listings.length, totalSalesCount: sales.length },
    contract: typeof body.contract === "string" ? body.contract : "",
    source: "arena",
  };
}

