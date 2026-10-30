import { apiBase } from "./protocol";

export type HyperPosition = {
  coin: string;
  size: number;
  entryPx: number;
  positionValue: number;
  unrealizedPnl: number;
  leverage: number;
  leverageType: string;
};

export type HyperAccount = {
  source: "hyperliquid" | "unavailable";
  address: string;
  accountValue: number;
  withdrawable: number;
  marginUsed: number;
  positions: HyperPosition[];
};

export async function fetchHyperAccount(address: string): Promise<HyperAccount | null> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 5000);
  try {
    const response = await fetch(
      `${apiBase()}/v1/accounts/hyperliquid?address=${encodeURIComponent(address)}`,
      { cache: "no-store", signal: controller.signal },
    );
    if (!response.ok) return null;
    const payload = (await response.json()) as Partial<HyperAccount>;
    if (payload.source !== "hyperliquid" && payload.source !== "unavailable") return null;
    return {
      source: payload.source,
      address: typeof payload.address === "string" ? payload.address : address,
      accountValue: Number(payload.accountValue) || 0,
      withdrawable: Number(payload.withdrawable) || 0,
      marginUsed: Number(payload.marginUsed) || 0,
      positions: Array.isArray(payload.positions) ? payload.positions : [],
    };
  } catch {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

export function formatUsd(value: number): string {
  if (!Number.isFinite(value)) return "—";
  return `$${value.toLocaleString("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })}`;
}

export function formatSignedUsd(value: number): string {
  if (!Number.isFinite(value)) return "—";
  const sign = value > 0 ? "+" : "";
  return `${sign}${formatUsd(value)}`;
}
