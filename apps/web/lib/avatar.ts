/** Deterministic look for one NEAR account. Same id, same ingot. */

export type HatKind = "none" | "bolt" | "ring";

export type AvatarSpec = {
  color: string;
  hat: HatKind;
  /** Four bits. Each bit lights one cell of the core panel. */
  core: number;
};

const PALETTE = [
  "#ff6b4a",
  "#ffd166",
  "#2ec4b6",
  "#c77dff",
  "#f4a261",
  "#ef476f",
  "#8ecae6",
  "#b5e48c",
];

export function avatarSpec(accountId: string): AvatarSpec {
  let acc = 2166136261;
  for (let i = 0; i < accountId.length; i += 1) {
    acc ^= accountId.charCodeAt(i);
    acc = Math.imul(acc, 16777619);
  }
  const u = acc >>> 0;
  const hats: HatKind[] = ["none", "bolt", "ring"];
  return {
    color: PALETTE[u % PALETTE.length],
    hat: hats[(u >>> 4) % hats.length],
    core: (u >>> 8) & 0b1111,
  };
}

export function displayName(accountId: string): string {
  const label = accountId.split(".")[0] ?? accountId;
  return label.slice(0, 1).toUpperCase() + label.slice(1);
}
