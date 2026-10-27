/** Same FNV mix as `fe2vi_core::default_skin`. Indexes only. No gameplay effect. */

export type SkinTraits = {
  body: number;
  face: number;
  role: number;
  hat: number;
  accessory: number;
  trail: number;
};

export const BODY_STYLES = 8;
export const FACE_STYLES = 4;
export const ROLE_OUTFITS = 6;
export const HAT_STYLES = 6;
export const ACCESSORIES = 6;
export const TRAILS = 4;

export const BODY_COLORS = [
  "#c45c26",
  "#8a939c",
  "#1ec8c0",
  "#f0c14a",
  "#d4543c",
  "#4d6a8a",
  "#e4d7c5",
  "#6e4b3a",
];

const HAT_NODES: Record<number, string[]> = {
  1: ["hat.1"],
  2: ["hat.2"],
  3: ["hat.3"],
  4: ["hat.4"],
  5: ["hat.5", "hat.5.tip"],
};

const ACCESSORY_NODES: Record<number, string[]> = {
  1: ["acc.1.L", "acc.1.R", "acc.1.band"],
  2: ["acc.2", "acc.2.handle"],
  3: ["acc.3.L", "acc.3.R", "acc.3.strap"],
  4: ["acc.4", "acc.4.strap"],
  5: ["acc.5"],
};

const ROLE_NODES: Record<number, string[]> = {
  0: ["hat.1"],
  1: ["hat.2", "acc.1.L", "acc.1.R", "acc.1.band"],
  2: ["hat.3", "hat.5", "hat.5.tip"],
  3: ["acc.2", "acc.2.handle"],
  4: ["hat.4"],
  5: ["acc.3.L", "acc.3.R", "acc.3.strap", "acc.4", "acc.4.strap"],
};

const OPTIONAL = new Set<string>([
  "face.0",
  "face.1",
  "face.2",
  "face.3",
  ...Object.values(HAT_NODES).flat(),
  ...Object.values(ACCESSORY_NODES).flat(),
]);

export type Mood = "trait" | "happy" | "alarm";

export function defaultSkin(accountId: string): SkinTraits {
  let acc = 2166136261;
  for (let i = 0; i < accountId.length; i += 1) {
    acc ^= accountId.charCodeAt(i);
    acc = Math.imul(acc, 16777619) >>> 0;
  }
  return {
    body: acc % BODY_STYLES,
    face: (acc >>> 3) % FACE_STYLES,
    role: (acc >>> 6) % ROLE_OUTFITS,
    hat: (acc >>> 10) % HAT_STYLES,
    accessory: (acc >>> 14) % ACCESSORIES,
    trail: (acc >>> 18) % TRAILS,
  };
}

export function visibleParts(skin: SkinTraits, mood: Mood): Set<string> {
  const face = mood === "happy" ? 1 : mood === "alarm" ? 2 : skin.face;
  const shown = new Set<string>([`face.${face}`]);
  for (const name of HAT_NODES[skin.hat] ?? []) shown.add(name);
  for (const name of ACCESSORY_NODES[skin.accessory] ?? []) shown.add(name);
  for (const name of ROLE_NODES[skin.role] ?? []) shown.add(name);
  return shown;
}

export function optionalPart(name: string): boolean {
  return OPTIONAL.has(name);
}
