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

export async function fetchSkins(): Promise<{ skins: Skin[]; source: "arena" | "fixture" }> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 4000);
  try {
    const response = await fetch(`${apiBase()}/v1/skins`, {
      cache: "no-store",
      signal: controller.signal,
    });
    if (!response.ok) return { skins: [], source: "fixture" };
    const body = (await response.json()) as { skins?: Skin[] };
    const rows = Array.isArray(body.skins) ? body.skins.filter((row) => row && typeof row.id === "string") : [];
    return { skins: rows, source: "arena" };
  } catch {
    return { skins: [], source: "fixture" };
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
