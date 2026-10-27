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

export type CameraShot = { eye: [number, number, number]; aim: [number, number, number]; fov?: number };

/** Six roles, shoulder to shoulder, facing the lawn camera. */
/**
 * The rig is about 1 unit tall. 1.75 keeps the head under the 3.2 sign and
 * lamp post, around shoulder height in the vault doorway. 5.3, and then
 * 5.3/1.5, still stood as tall as that sign.
 */
export const AGENT_SCALE = 1.75;
/** Head height in world units, the same proportion as the old 5.3 body. */
export const AGENT_HEAD = (3.4 / 5.3) * AGENT_SCALE;

export const LINEUP_SPOTS: Array<[number, number]> = [
  [-11.25, 0],
  [-6.75, 0],
  [-2.25, 0],
  [2.25, 0],
  [6.75, 0],
  [11.25, 0],
];

export const LINEUP_SHOT: CameraShot = { eye: [0, 3.8 / 1.5, 18], aim: [0, 2.7 / 1.5, 0], fov: 50 };

/**
 * Wider street frame. About 11 units back from the agents in front of the
 * vault doorway, 30° vertical field, so the portico, the sign, and a lamp
 * stay in frame with the four bodies.
 */
export const STREET_SHOT: CameraShot = { eye: [-43.5, 1.6, 45.6], aim: [-42.2, 2.2, 33.2], fov: 30 };

export const FACE_SHOT: CameraShot = { eye: [0.15, 3.55 / 1.5, 3.4 / 1.5], aim: [0, 3.35 / 1.5, 0], fov: 28 };

export const WALK_SHOT: CameraShot = { eye: [0, 2.4 / 1.5, 14 / 1.5], aim: [0, 2.2 / 1.5, 0], fov: 42 };

/** Eye height in the open southeast corner, looking across the floor. The east wall is hidden. */
function acrossRoom(x: number, z: number): CameraShot {
  return { eye: [x + 2.7, 1.62, z + 2.15], aim: [x - 1.55, 0.82, z - 1.45], fov: 64 };
}

export const INTERIOR_SHOTS: Record<string, CameraShot> = {
  "vault-quarter": acrossRoom(-42, 18),
  "trading-village": acrossRoom(46, 14),
  "war-room": acrossRoom(12, 40),
  "skin-forge": acrossRoom(-38, -24),
  "cup-stadium": acrossRoom(-8, -34),
  "launch-pier": { eye: [62, 8, -12], aim: [50, 3, -28] },
  "card-club": { eye: [-42, 8, 12], aim: [-54, 3, -2] },
  "oracle-plaza": { eye: [32, 8, 4], aim: [20, 3, -10] },
};

/** Where an agent stands while a room camera is active, clear of that corner. */
const CAMERA_PARK: Record<string, [number, number]> = {
  "vault-quarter": [-48, 34],
  "trading-village": [40, 30],
  "war-room": [6, 56],
  "skin-forge": [-44, -8],
  "cup-stadium": [-14, -18],
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

/** Road ring. Stops stay on the lawn in front of each plot, outside the rooms. */
export const WALK_LOOP: Array<[number, number]> = [
  [-54, -2],
  [-38, -8],
  [-8, -18],
  [50, -12],
  [20, 4],
  [46, 30],
  [12, 56],
  [-42, 34],
];

export function placeAgent(index: number, time: number, paused: boolean) {
  const phase = index * 1.7;
  const home = WALK_LOOP[index % WALK_LOOP.length];
  if (paused) {
    return { x: home[0], y: 0, z: home[1], rot: 0, speed: 0 };
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
  const dist = Math.hypot(dx, dz);
  const ease = 6 * t * (1 - t);
  return {
    x: from[0] + dx * smooth,
    y: 0,
    z: from[1] + dz * smooth,
    rot: Math.atan2(dx, dz),
    speed: dist * 0.28 * ease,
  };
}

export function poseClearOf(
  pose: { x: number; y: number; z: number; rot: number },
  focusId: string | null,
  index: number,
) {
  if (!focusId) return pose;
  const shot = INTERIOR_SHOTS[focusId];
  const park = CAMERA_PARK[focusId];
  if (!shot || !park) return pose;
  const dx = pose.x - shot.eye[0];
  const dz = pose.z - shot.eye[2];
  if (dx * dx + dz * dz > 64) return pose;
  return { ...pose, x: park[0] + (index % 3) * 1.8, z: park[1] };
}
