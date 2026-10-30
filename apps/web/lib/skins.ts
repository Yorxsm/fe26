import { apiBase } from "./protocol";
import {
  BACK_ITEMS,
  EMBLEMS,
  EYE_STYLES,
  FACE_ITEMS,
  HEADWEAR,
  type SkinTraits,
} from "./skin";

export type Skin = {
  id: string;
  name: string;
  role: number;
  paint: number;
  screen: number;
  trail: number;
  rarity: "Common" | "Rare" | "Mythic" | string;
  unlockPoints: number;
  description: string;
};

export const FALLBACK_SKINS: Skin[] = [
  { id: "ingot", name: "Ingot", role: 0, paint: 0, screen: 0, trail: 0, rarity: "Common", unlockPoints: 0, description: "The starting body. Steel paint, amber screen, sparks." },
  { id: "safecracker", name: "Safecracker", role: 0, paint: 6, screen: 3, trail: 0, rarity: "Common", unlockPoints: 50, description: "Rust paint and a signal screen. Unlocked by playing the vault." },
  { id: "trader", name: "Trader", role: 1, paint: 1, screen: 1, trail: 1, rarity: "Rare", unlockPoints: 100, description: "Violet paint, magenta screen, coin trail." },
  { id: "hacker", name: "Hacker", role: 2, paint: 2, screen: 2, trail: 2, rarity: "Rare", unlockPoints: 150, description: "Night paint, cyan screen, glitch trail." },
  { id: "auditor", name: "Auditor", role: 3, paint: 3, screen: 6, trail: 3, rarity: "Rare", unlockPoints: 200, description: "Paper paint, white screen, smoke trail." },
  { id: "builder", name: "Builder", role: 4, paint: 4, screen: 4, trail: 0, rarity: "Rare", unlockPoints: 250, description: "Copper paint, mint screen, sparks." },
  { id: "scout", name: "Scout", role: 5, paint: 5, screen: 5, trail: 2, rarity: "Rare", unlockPoints: 300, description: "Olive paint, lime screen, glitch trail." },
  { id: "gold-leaf", name: "Gold leaf", role: 0, paint: 8, screen: 0, trail: 1, rarity: "Mythic", unlockPoints: 500, description: "Gold metal, amber screen, coin trail." },
  { id: "season-crown", name: "Season crown", role: 3, paint: 9, screen: 6, trail: 1, rarity: "Mythic", unlockPoints: 750, description: "Chrome finish, white screen, coin trail." },
];

export async function fetchSkins(): Promise<{ skins: Skin[]; source: "arena" | "fixture" }> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 4000);
  try {
    const response = await fetch(`${apiBase()}/v1/skins`, {
      cache: "no-store",
      signal: controller.signal,
    });
    if (!response.ok) return { skins: FALLBACK_SKINS, source: "fixture" };
    const body = (await response.json()) as { skins?: Skin[] };
    const rows = Array.isArray(body.skins) ? body.skins.filter((row) => row && typeof row.id === "string") : [];
    return rows.length > 0 ? { skins: rows, source: "arena" } : { skins: FALLBACK_SKINS, source: "fixture" };
  } catch {
    return { skins: FALLBACK_SKINS, source: "fixture" };
  } finally {
    clearTimeout(timer);
  }
}

export function skinTraits(skin: Skin): SkinTraits {
  const role = ((skin.role % 6) + 6) % 6;
  return {
    paint: skin.paint,
    screen: skin.screen,
    eyes: role % EYE_STYLES,
    headwear: role % HEADWEAR,
    face_item: role % FACE_ITEMS,
    back_item: role % BACK_ITEMS,
    emblem: role % EMBLEMS,
    trail: skin.trail,
    role,
  };
}

export function unlockLabel(skin: Skin): string {
  return skin.unlockPoints === 0 ? "Starter" : `${skin.unlockPoints} points`;
}
