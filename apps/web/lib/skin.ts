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

/** Safecracker, Trader, Hacker, Auditor, Builder, Scout. Baked into each body. */
export const ROLE_COLORS = ["#9aa0a6", "#7a4fc4", "#1d2a3a", "#e7e4dc", "#e0742a", "#6b7a3a"];

export const ROLE_BODIES = [
  "role.safecracker",
  "role.trader",
  "role.hacker",
  "role.auditor",
  "role.builder",
  "role.scout",
];

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

/** null means the node is not a role part. Otherwise whether this authored name should show. */
export function roleVisible(label: string, skin: SkinTraits, mood: Mood): boolean | null {
  if (!label.startsWith("role.")) return null;
  const body = ROLE_BODIES[skin.role] ?? ROLE_BODIES[0];
  if (!label.startsWith(`${body}.`)) return false;
  const face = label.match(/\.face\.(\d+)$/);
  if (!face) return true;
  const want = mood === "happy" ? 1 : mood === "alarm" ? 2 : skin.face % FACE_STYLES;
  return Number(face[1]) === want;
}
