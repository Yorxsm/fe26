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

const KITS: Record<string, { jersey: string; trim: string }> = {
  rookie: { jersey: "#3a3a3e", trim: "#8b8a86" },
  bronze: { jersey: "#8a5a32", trim: "#c94a1a" },
  silver: { jersey: "#b7b8be", trim: "#2fb86a" },
  gold: { jersey: "#c9a227", trim: "#8a3110" },
  platinum: { jersey: "#d5dde3", trim: "#5f7d8c" },
  diamond: { jersey: "#7eb6d6", trim: "#1f8a4e" },
};

const BOOTS = ["#141416", "#c94a1a", "#1f8a4e", "#5f7d8c"];

export function playerSeed(tokenId: string): number {
  let hash = 2166136261;
  for (let index = 0; index < tokenId.length; index += 1) {
    hash ^= tokenId.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

export function playerMark(tokenId: string, tier: string | null): {
  jersey: string;
  trim: string;
  boots: string;
  number: number;
  visor: boolean;
} {
  const seed = playerSeed(tokenId);
  const kit = KITS[(tier ?? "").trim().toLowerCase()] ?? KITS.rookie;
  return {
    jersey: kit.jersey,
    trim: kit.trim,
    boots: BOOTS[seed % BOOTS.length],
    number: (seed % 99) + 1,
    visor: (seed & 8) === 8,
  };
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
