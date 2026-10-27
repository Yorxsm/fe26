/** Same FNV mix as `fe2vi_core::default_skin`. Indexes only. No gameplay effect. */

export type SkinTraits = {
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

export const PAINTS = 12;
export const SCREENS = 8;
export const EYE_STYLES = 6;
export const HEADWEAR = 8;
export const FACE_ITEMS = 4;
export const BACK_ITEMS = 4;
export const EMBLEMS = 6;
export const TRAILS = 4;
export const ROLE_OUTFITS = 6;

/** Steel, violet, night, paper, copper, olive, rust, teal, gold, chrome, matte black, camo. */
export const PAINT_COLORS = [
  "#9aa0a6",
  "#7a4fc4",
  "#1d2a3a",
  "#e7e4dc",
  "#e0742a",
  "#6b7a3a",
  "#c45c26",
  "#1ec8c0",
  "#d4a017",
  "#d5d8de",
  "#141416",
  "#4a5a3a",
];

export const SCREEN_COLORS = [
  "#ffc928",
  "#e04bff",
  "#2fe6e6",
  "#c94a1a",
  "#2fb86a",
  "#9cff3a",
  "#d4d2cc",
  "#ff4d6a",
];

export const PAINT_NAMES = [
  "Steel",
  "Violet",
  "Night",
  "Paper",
  "Copper",
  "Olive",
  "Rust",
  "Teal",
  "Gold",
  "Chrome",
  "Matte black",
  "Camo",
];

export const SCREEN_NAMES = ["Amber", "Magenta", "Cyan", "Signal", "Mint", "Lime", "White", "Rose"];
export const EYE_NAMES = ["Round", "Pill", "Visor", "Heart", "Sleepy", "Pixel"];
export const HEADWEAR_NAMES = ["Beanie", "Headset", "Hood", "Hard hat", "Goggles", "Antenna", "Crown", "Cap"];
export const FACE_ITEM_NAMES = ["Brow bar", "Monocle", "Mask", "Mic"];
export const BACK_ITEM_NAMES = ["Pack", "Jetpack", "Cape", "Array"];
export const EMBLEM_NAMES = ["Key", "Coin", "Glyph", "Shield", "Badge", "Compass"];
export const TRAIL_NAMES = ["Sparks", "Coins", "Glitch", "Smoke"];
export const ROLE_NAMES = ["Safecracker", "Trader", "Hacker", "Auditor", "Builder", "Scout"];

/** Safecracker, Trader, Hacker, Auditor, Builder, Scout. Kept so the city still dresses a role. */
export const ROLE_COLORS = ["#9aa0a6", "#7a4fc4", "#1d2a3a", "#e7e4dc", "#e0742a", "#6b7a3a"];

export const ROLE_BODIES = [
  "role.safecracker",
  "role.trader",
  "role.hacker",
  "role.auditor",
  "role.builder",
  "role.scout",
];

const HEADWEAR_NODES = [
  ["role.safecracker.beanie", "role.safecracker.keyhole"],
  ["role.trader.band"],
  ["role.hacker.hood", "role.hacker.string.L", "role.hacker.string.R"],
  ["role.builder.hat"],
  ["role.scout.goggle.L", "role.scout.goggle.R", "role.scout.goggle.strap", "role.scout.goggle.bridge"],
  ["body.antenna", "body.antenna.tip"],
  ["headwear.crown", "headwear.crown.band"],
  ["headwear.cap", "headwear.brim"],
];

export type Mood = "trait" | "happy" | "alarm";

export type PaintFinish = { color: string; metal: number; rough: number };

export function paintFinish(index: number): PaintFinish {
  const paint = ((index % PAINTS) + PAINTS) % PAINTS;
  const metal = paint === 8 ? 0.85 : paint === 9 ? 0.92 : 0.3;
  const rough = paint === 10 ? 0.92 : paint === 8 || paint === 9 ? 0.22 : paint === 11 ? 0.8 : 0.5;
  return { color: PAINT_COLORS[paint], metal, rough };
}

export function screenColor(index: number): string {
  return SCREEN_COLORS[((index % SCREENS) + SCREENS) % SCREENS];
}

export function defaultSkin(accountId: string): SkinTraits {
  let acc = 2166136261;
  for (let i = 0; i < accountId.length; i += 1) {
    acc ^= accountId.charCodeAt(i);
    acc = Math.imul(acc, 16777619) >>> 0;
  }
  return {
    paint: acc % PAINTS,
    screen: (acc >>> 4) % SCREENS,
    eyes: (acc >>> 7) % EYE_STYLES,
    headwear: (acc >>> 10) % HEADWEAR,
    face_item: (acc >>> 13) % FACE_ITEMS,
    back_item: (acc >>> 15) % BACK_ITEMS,
    emblem: (acc >>> 17) % EMBLEMS,
    trail: (acc >>> 20) % TRAILS,
    role: (acc >>> 22) % ROLE_OUTFITS,
  };
}

/** Six readable roles for the lineup plate. Indexes stay inside the allowlist. */
export function lineupSkin(role: number): SkinTraits {
  const index = ((role % ROLE_OUTFITS) + ROLE_OUTFITS) % ROLE_OUTFITS;
  return {
    paint: [0, 1, 2, 3, 4, 5][index],
    screen: [0, 1, 2, 3, 4, 5][index],
    eyes: [0, 1, 2, 0, 3, 5][index],
    headwear: [0, 1, 2, 5, 3, 4][index],
    face_item: [0, 3, 2, 1, 0, 0][index],
    back_item: [1, 2, 3, 1, 2, 0][index],
    emblem: index,
    trail: index % TRAILS,
    role: index,
  };
}

export type SkinStats = {
  wins: number;
  heistsCracked: number;
  timesCaught: number;
  seasonsPlayed: number;
  seasonTitles: number;
};

/** Sample record used by the forge and the profile card. Not a chain read. */
export const DEMO_SKIN_STATS: SkinStats = {
  wins: 7,
  heistsCracked: 3,
  timesCaught: 2,
  seasonsPlayed: 1,
  seasonTitles: 0,
};

export const EARNED_TRAIT_SPECS: Array<{
  id: string;
  slot: string;
  name: string;
  goal: number;
  unit: string;
  progressOf: (stats: SkinStats) => number;
}> = [
  { id: "gold-leaf", slot: "paint", name: "Gold leaf", goal: 10, unit: "wins", progressOf: (stats) => stats.wins },
  { id: "cracked-vault", slot: "emblem", name: "Cracked vault", goal: 5, unit: "heists", progressOf: (stats) => stats.heistsCracked },
  { id: "molten-trail", slot: "trail", name: "Molten trail", goal: 25, unit: "wins", progressOf: (stats) => stats.wins },
  {
    id: "season-crown",
    slot: "headwear",
    name: "Season crown",
    goal: 1,
    unit: "season titles",
    progressOf: (stats) => stats.seasonTitles,
  },
];

export function rarity(traits: SkinTraits): "Common" | "Rare" | "Mythic" {
  const score =
    (traits.paint % PAINTS === 8 || traits.paint % PAINTS === 9 ? 2 : 0) +
    (traits.headwear % HEADWEAR === 6 ? 2 : 0) +
    (traits.screen % SCREENS === 6 ? 1 : 0);
  if (score >= 3) return "Mythic";
  if (score >= 1) return "Rare";
  return "Common";
}

function matches(label: string, name: string) {
  return label === name || label.startsWith(`${name}.`);
}

/** null means this node is not a swappable part. */
export function nodeVisible(label: string, skin: SkinTraits, _mood: Mood): boolean | null {
  if (label.startsWith("body.face.") || label === "body.face") return false;
  for (let index = 0; index < HEADWEAR_NODES.length; index += 1) {
    if (HEADWEAR_NODES[index].some((name) => matches(label, name))) return index === skin.headwear % HEADWEAR;
  }
  if (label.startsWith("faceitem.")) return Number(label.split(".")[1]) === skin.face_item % FACE_ITEMS;
  if (label.startsWith("back.")) return Number(label.split(".")[1]) === skin.back_item % BACK_ITEMS;
  if (label.startsWith("emblem.")) return Number(label.split(".")[1]) === skin.emblem % EMBLEMS;
  if (label.startsWith("role.scout.pack") || label.startsWith("role.scout.strap")) return false;
  if (label.startsWith("role.")) {
    const body = ROLE_BODIES[skin.role % ROLE_OUTFITS] ?? ROLE_BODIES[0];
    return label.startsWith(`${body}.`);
  }
  return null;
}
