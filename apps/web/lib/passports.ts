export type PassportRow = {
  tokenId: string;
  owner: string;
  contract: string;
  tier: string | null;
  career: { xp?: number; wins?: number; matches?: number } | null;
  strategyHash: string | null;
  strategyVersion: number | null;
  priceNear?: string | null;
};

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
