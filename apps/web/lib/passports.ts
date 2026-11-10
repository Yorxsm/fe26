export type CareerNumbers = {
  xp?: number;
  wins?: number;
  losses?: number;
  draws?: number;
  matches?: number;
};

export type PassportRow = {
  tokenId: string;
  owner: string;
  contract: string;
  tier: string | null;
  career: CareerNumbers | null;
  strategyHash: string | null;
  strategyVersion: number | null;
  priceNear?: string | null;
};

export function tierFrame(tier: string | null): string {
  switch ((tier ?? "").trim().toLowerCase()) {
    case "bronze":
      return "#c45c26";
    case "silver":
      return "#d5d8de";
    case "gold":
      return "#d4a017";
    case "platinum":
      return "#e7e4dc";
    case "diamond":
      return "#1ec8c0";
    default:
      return "#3a3a3e";
  }
}

export function listedOffer(
  tokenId: string,
  listings: Array<{ tokenId?: string; priceNear?: string | null; priceYocto?: string | null; listed?: boolean }>,
): { priceNear: string; priceYocto: string | null } | null {
  const row = listings.find((item) => item.tokenId === tokenId && item.listed !== false && item.priceNear);
  if (!row?.priceNear) return null;
  return { priceNear: row.priceNear, priceYocto: row.priceYocto ?? null };
}

export function marketStats(
  rows: Array<{ tokenId: string }>,
  listings: Array<{ tokenId?: string; priceNear?: string | null; priceYocto?: string | null; listed?: boolean }>,
): { total: number; listed: number; floor: string | null } {
  let listed = 0;
  let floor: string | null = null;
  let floorValue = Number.POSITIVE_INFINITY;
  for (const row of rows) {
    const offer = listedOffer(row.tokenId, listings);
    if (!offer) continue;
    listed += 1;
    const value = Number(offer.priceNear);
    if (!Number.isFinite(value) || value >= floorValue) continue;
    floorValue = value;
    floor = offer.priceNear;
  }
  return { total: rows.length, listed, floor: listed > 0 ? floor : null };
}

export type MarketSort = "id" | "xp" | "price";

export function sortMarket<T extends { tokenId: string; career?: CareerNumbers | null }>(
  rows: T[],
  listings: Array<{ tokenId?: string; priceNear?: string | null; priceYocto?: string | null; listed?: boolean }>,
  sort: MarketSort,
): T[] {
  return [...rows].sort((left, right) => {
    if (sort === "xp") {
      const delta = (right.career?.xp ?? -1) - (left.career?.xp ?? -1);
      if (delta !== 0) return delta;
    }
    if (sort === "price") {
      const leftOffer = listedOffer(left.tokenId, listings);
      const rightOffer = listedOffer(right.tokenId, listings);
      const leftValue = leftOffer ? Number(leftOffer.priceNear) : Number.POSITIVE_INFINITY;
      const rightValue = rightOffer ? Number(rightOffer.priceNear) : Number.POSITIVE_INFINITY;
      if (leftValue !== rightValue) return leftValue - rightValue;
    }
    return left.tokenId.localeCompare(right.tokenId);
  });
}

export function recordLine(career: CareerNumbers | null | undefined): string | null {
  if (!career) return null;
  const parts: string[] = [];
  if (typeof career.wins === "number") parts.push(`${career.wins}W`);
  if (typeof career.losses === "number") parts.push(`${career.losses}L`);
  if (typeof career.draws === "number") parts.push(`${career.draws}D`);
  return parts.length ? parts.join(" ") : null;
}

function asNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function careerFrom(value: unknown): CareerNumbers | null {
  if (!value || typeof value !== "object") return null;
  const row = value as Record<string, unknown>;
  const career: CareerNumbers = {};
  const xp = asNumber(row.xp);
  const wins = asNumber(row.wins);
  const losses = asNumber(row.losses);
  const draws = asNumber(row.draws);
  const matches = asNumber(row.matches);
  if (xp !== null) career.xp = xp;
  if (wins !== null) career.wins = wins;
  if (losses !== null) career.losses = losses;
  if (draws !== null) career.draws = draws;
  if (matches !== null) career.matches = matches;
  return Object.keys(career).length ? career : null;
}

export function testnetAccountUrl(account: string | null): string | null {
  if (!account) return null;
  const id = account.trim();
  if (!/^[a-z0-9._-]+\.testnet$/.test(id)) return null;
  return `https://testnet.nearblocks.io/address/${id}`;
}

export type PassportDetail = {
  owner: string | null;
  contract: string | null;
  contractUrl: string | null;
  boundAgent: string | null;
  tier: string | null;
  career: CareerNumbers | null;
  onchainCareer: CareerNumbers | null;
  strategyVersionCount: number | null;
  strategyHash: string | null;
  strategyUpdatedAt: string | null;
};

export function passportDetail(body: unknown): PassportDetail {
  const empty: PassportDetail = {
    owner: null,
    contract: null,
    contractUrl: null,
    boundAgent: null,
    tier: null,
    career: null,
    onchainCareer: null,
    strategyVersionCount: null,
    strategyHash: null,
    strategyUpdatedAt: null,
  };
  if (!body || typeof body !== "object") return empty;
  const root = body as Record<string, unknown>;
  const passport = root.passport;
  if (!passport || typeof passport !== "object") return empty;
  const row = passport as Record<string, unknown>;
  const owner = typeof row.ownerNear === "string" && row.ownerNear.trim() ? row.ownerNear.trim() : null;
  const contract = typeof root.contract === "string" && root.contract.trim() ? root.contract.trim() : null;
  const bound = typeof row.boundAgent === "string" && row.boundAgent.trim()
    ? row.boundAgent.trim()
    : typeof row.agentId === "string" && row.agentId.trim()
      ? row.agentId.trim()
      : null;
  const hash = typeof row.strategyHash === "string" && row.strategyHash.trim() ? row.strategyHash.trim() : null;
  const updated = typeof row.strategyUpdatedAt === "string" && row.strategyUpdatedAt.trim() ? row.strategyUpdatedAt.trim() : null;
  return {
    owner,
    contract,
    contractUrl: testnetAccountUrl(contract),
    boundAgent: bound,
    tier: typeof row.tier === "string" && row.tier.trim() ? row.tier.trim() : null,
    career: careerFrom(row.career),
    onchainCareer: careerFrom(row.onchainCareer ?? root.onchainCareer),
    strategyVersionCount: asNumber(row.strategyVersionCount),
    strategyHash: hash,
    strategyUpdatedAt: updated,
  };
}

export function accountFromStorage(entries: Array<[string, string | null]>): string | null {
  for (const [key, value] of entries) {
    if (!value || !/wallet|near/i.test(key)) continue;
    const direct = value.trim();
    if (direct.endsWith(".testnet") && !direct.includes(" ") && !direct.startsWith("{")) return direct;
    try {
      const parsed = JSON.parse(value) as { accountId?: unknown; account_id?: unknown };
      const id = parsed.accountId ?? parsed.account_id;
      if (typeof id === "string" && id.endsWith(".testnet")) return id;
    } catch {
      continue;
    }
  }
  return null;
}

export function splitByOwner<T extends { owner: string }>(rows: T[], owner: string): { owned: T[]; other: T[] } {
  const want = owner.trim().toLowerCase();
  if (!want) return { owned: [], other: rows };
  const owned: T[] = [];
  const other: T[] = [];
  for (const row of rows) {
    if (row.owner.trim().toLowerCase() === want) owned.push(row);
    else other.push(row);
  }
  return { owned, other };
}

export function priceFor(tokenId: string, listings: Array<{ tokenId?: string; priceNear?: string | null; listed?: boolean }>): string | null {
  const row = listings.find((item) => item.tokenId === tokenId && item.priceNear && item.listed !== false);
  return row?.priceNear ?? null;
}
