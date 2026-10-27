import { displayName } from "./avatar";
import { defaultSkin, type SkinTraits } from "./skin";

export type District = {
  id: string;
  name: string;
  summary: string;
  status: "live" | "soon";
  href: string;
  station: string;
  color: string;
  x: number;
  z: number;
};

const LAYOUT: Array<Omit<District, "x" | "z">> = [
  {
    id: "vault-quarter",
    name: "Vault Quarter",
    summary: "Heist Night. Timed vault rounds inside platform caps.",
    status: "live",
    href: "/games/heist-night-template",
    station: "Vault Bank",
    color: "#ff6b4a",
  },
  {
    id: "cup-stadium",
    name: "Cup Stadium",
    summary: "Trading Cup. A short public tape.",
    status: "live",
    href: "/games/trading-cup-template",
    station: "Trading Cup Stadium",
    color: "#ffd166",
  },
  {
    id: "trading-village",
    name: "Trading Village",
    summary: "Open tape, rolling boards, and follow.",
    status: "live",
    href: "/village",
    station: "Trading Market 24/7",
    color: "#2ec4b6",
  },
  {
    id: "war-room",
    name: "War Room",
    summary: "A breach challenge on this contract only.",
    status: "live",
    href: "/war-room",
    station: "Hacker War Room",
    color: "#ef476f",
  },
  {
    id: "skin-forge",
    name: "Skin Forge",
    summary: "Cosmetic skins. One NEAR. No effect on the game.",
    status: "live",
    href: "",
    station: "Skin Forge",
    color: "#ff7a1a",
  },
  {
    id: "oracle-plaza",
    name: "Oracle Plaza",
    summary: "Coming soon.",
    status: "soon",
    href: "",
    station: "Coming Soon",
    color: "#8ecae6",
  },
  {
    id: "launch-pier",
    name: "Launch Pier",
    summary: "Coming soon.",
    status: "soon",
    href: "",
    station: "Coming Soon",
    color: "#f4a261",
  },
  {
    id: "card-club",
    name: "Card Club",
    summary: "Coming soon.",
    status: "soon",
    href: "",
    station: "Coming Soon",
    color: "#b5e48c",
  },
];

/** glTF meters from apps/web/public/models/layout.json. Y is up, so z is the file's third value. */
const ROOM_XZ: Record<string, [number, number]> = {
  "vault-quarter": [-42, 18],
  "cup-stadium": [-8, -42],
  "trading-village": [46, 14],
  "war-room": [12, 40],
  "skin-forge": [-38, -24],
  "launch-pier": [50, -28],
  "card-club": [-54, -2],
  "oracle-plaza": [20, -10],
};

export type CameraShot = { eye: [number, number, number]; aim: [number, number, number] };

/** Inside the doorway, looking at the back wall. Plots stay outside the fence. */
export const INTERIOR_SHOTS: Record<string, CameraShot> = {
  "vault-quarter": { eye: [-40.6, 1.72, 24.4], aim: [-42.2, 1.3, 16.6] },
  "trading-village": { eye: [47.4, 1.72, 20.4], aim: [45.8, 1.3, 12.6] },
  "war-room": { eye: [13.4, 1.72, 46.4], aim: [11.8, 1.3, 38.6] },
  "skin-forge": { eye: [-36.6, 1.72, -17.6], aim: [-38.2, 1.2, -25.4] },
  "cup-stadium": { eye: [-6.6, 1.8, -27.6], aim: [-8.2, 1.25, -35.4] },
  "launch-pier": { eye: [58, 5, -16], aim: [50, 2, -28] },
  "card-club": { eye: [-46, 5, 8], aim: [-54, 2, -2] },
  "oracle-plaza": { eye: [28, 5, 0], aim: [20, 2, -10] },
};

/** Front lawn, looking at the facade. */
export const EXTERIOR_SHOTS: Record<string, CameraShot> = {
  "vault-quarter": { eye: [-42, 10, 54], aim: [-42, 6, 18] },
  "trading-village": { eye: [46, 10, 50], aim: [46, 6, 14] },
  "war-room": { eye: [12, 12, 72], aim: [12, 8, 40] },
  "skin-forge": { eye: [-38, 9, -2], aim: [-38, 4, -24] },
  "cup-stadium": { eye: [-8, 14, -14], aim: [-8, 4, -42] },
  "launch-pier": { eye: [50, 12, -4], aim: [50, 4, -28] },
  "card-club": { eye: [-54, 12, 18], aim: [-54, 4, -2] },
  "oracle-plaza": { eye: [20, 12, 10], aim: [20, 4, -10] },
};

export const DISTRICTS: District[] = LAYOUT.map((district) => {
  const [x, z] = ROOM_XZ[district.id] ?? [0, 0];
  return { ...district, x, z };
});

export type CityAgent = {
  accountId: string;
  name: string;
  skin: SkinTraits;
};

export const CITY_AGENTS: CityAgent[] = [
  "mason.village.fe2vi.testnet",
  "scout.vault.fe2vi.testnet",
  "tape.cup.fe2vi.testnet",
  "pike.war.fe2vi.testnet",
  "quill.forge.fe2vi.testnet",
  "nori.market.fe2vi.testnet",
  "brim.stadium.fe2vi.testnet",
  "kiln.forge.fe2vi.testnet",
  "wren.scout.fe2vi.testnet",
  "soot.vault.fe2vi.testnet",
].map((accountId) => ({
  accountId,
  name: displayName(accountId),
  skin: defaultSkin(accountId),
}));

/** Road ring, with a step through each live doorway (the +z face). */
export const WALK_LOOP: Array<[number, number]> = [
  [-54, -2],
  [-38, -24],
  [-38, -16],
  [-38, -24],
  [-8, -42],
  [-8, -32],
  [-8, -42],
  [50, -28],
  [20, -10],
  [46, 14],
  [46, 22],
  [46, 14],
  [12, 40],
  [12, 48],
  [12, 40],
  [-42, 18],
  [-42, 26],
  [-42, 18],
];

export function placeAgent(index: number, time: number, paused: boolean) {
  const phase = index * 1.7;
  const home = WALK_LOOP[index % WALK_LOOP.length];
  if (paused) {
    return { x: home[0], y: 0, z: home[1], rot: 0 };
  }
  const travel = time * 0.28 + phase;
  const span = WALK_LOOP.length;
  const wrapped = ((travel % span) + span) % span;
  const slot = Math.floor(wrapped) % span;
  const t = wrapped - Math.floor(wrapped);
  const smooth = t * t * (3 - 2 * t);
  const from = WALK_LOOP[slot];
  const to = WALK_LOOP[(slot + 1) % span];
  const dx = to[0] - from[0];
  const dz = to[1] - from[1];
  return {
    x: from[0] + dx * smooth,
    y: 0,
    z: from[1] + dz * smooth,
    rot: Math.atan2(dx, dz),
  };
}
