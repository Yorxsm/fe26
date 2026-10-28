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
    href: "/games/heist-night",
    station: "Vault Bank",
    color: "#c94a1a",
  },
  {
    id: "cup-stadium",
    name: "Cup Stadium",
    summary: "Trading Cup. A short public tape.",
    status: "live",
    href: "/games/trading-cup",
    station: "Trading Cup Stadium",
    color: "#8f7a4a",
  },
  {
    id: "trading-village",
    name: "Trading Village",
    summary: "Mock perps and stocks. Funding, PnL, and a leaderboard wall.",
    status: "live",
    href: "/village",
    station: "Trading Floor",
    color: "#2f8a56",
  },
  {
    id: "war-room",
    name: "War Room",
    summary: "Contract, web, and crypto sandboxes we host. Nothing outside this arena.",
    status: "live",
    href: "/war-room",
    station: "Hacker War Room",
    color: "#8b4548",
  },
  {
    id: "skin-forge",
    name: "Lobby",
    summary: "The entry hall. Boards for every game. Skins sit in a side alcove.",
    status: "live",
    href: "/lobby",
    station: "Lobby",
    color: "#a85a32",
  },
  {
    id: "oracle-plaza",
    name: "Oracle Plaza",
    summary: "Coming soon.",
    status: "soon",
    href: "",
    station: "Coming Soon",
    color: "#5e6a70",
  },
  {
    id: "launch-pier",
    name: "Launch Pier",
    summary: "Coming soon.",
    status: "soon",
    href: "",
    station: "Coming Soon",
    color: "#6d5b48",
  },
  {
    id: "card-club",
    name: "Card Club",
    summary: "Coming soon.",
    status: "soon",
    href: "",
    station: "Coming Soon",
    color: "#4d5c4a",
  },
];

/** glTF meters from apps/web/public/models/layout.json. Y is up, so z is the file's third value. */
export const ROOM_XZ: Record<string, [number, number]> = {
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
export const STREET_SHOT: CameraShot = { eye: [-43.6, 1.65, 46.2], aim: [-42.0, 2.55, 32.8], fov: 35 };

export const FACE_SHOT: CameraShot = { eye: [0.15, 3.55 / 1.5, 3.4 / 1.5], aim: [0, 3.35 / 1.5, 0], fov: 28 };

export const WALK_SHOT: CameraShot = { eye: [0, 2.4 / 1.5, 14 / 1.5], aim: [0, 2.2 / 1.5, 0], fov: 42 };

/**
 * Inside the authored room. The shell runs x ±7.2, z ±6.2, ceiling y 4.4.
 * The eye sits south of center, clear of the door lintel and the side walls.
 */
function insideRoom(x: number, z: number): CameraShot {
  return { eye: [x + 0.15, 1.62, z + 4.35], aim: [x - 0.15, 1.4, z - 2.6], fov: 50 };
}

export const INTERIOR_SHOTS: Record<string, CameraShot> = {
  "vault-quarter": insideRoom(-42, 18),
  "trading-village": insideRoom(46, 14),
  "war-room": insideRoom(12, 40),
  // Wide enough that the four signs and the crew sit inside both 1280×800 and a phone.
  "skin-forge": { eye: [-38, 2.4, -18.4], aim: [-38, 2.75, -28.6], fov: 46 },
  // The hanging board sits above the standard eye. This aim keeps the whole scoreboard in the 1280×800 frame.
  "cup-stadium": { eye: [-7.85, 1.7, -37.65], aim: [-8.15, 2.35, -44.2], fov: 48 },
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

/** Front lawn, close enough that the facade fills the frame. */
export const EXTERIOR_SHOTS: Record<string, CameraShot> = {
  "vault-quarter": { eye: [-42, 6.5, 44], aim: [-42, 3.4, 20], fov: 42 },
  "trading-village": { eye: [46, 6.5, 38], aim: [46, 3.2, 16], fov: 42 },
  "war-room": { eye: [12, 6.5, 56], aim: [12, 3.2, 40], fov: 40 },
  "skin-forge": { eye: [-38, 6, -8], aim: [-38, 2.8, -22], fov: 42 },
  "cup-stadium": { eye: [-8, 7, -26], aim: [-8, 3, -40], fov: 40 },
  "launch-pier": { eye: [50, 12, -4], aim: [50, 4, -28] },
  "card-club": { eye: [-54, 12, 18], aim: [-54, 4, -2] },
  "oracle-plaza": { eye: [20, 12, 10], aim: [20, 4, -10] },
};

/** Authored path for the opening. The first shot stays on the road, outside every shell. */
export const CUTSCENE_SHOTS: CameraShot[] = [
  { eye: [-50, 2.35, 52], aim: [-42, 1.55, 33], fov: 36 },
  INTERIOR_SHOTS["vault-quarter"],
  INTERIOR_SHOTS["trading-village"],
  INTERIOR_SHOTS["war-room"],
  INTERIOR_SHOTS["cup-stadium"],
  { eye: [-38.1, 1.85, -19.35], aim: [-38, 1.55, -28.2], fov: 44 },
];

export type CastPose = { index: number; x: number; z: number; rot: number; speed: number; clip?: "idle" | "walk" | "celebrate" };

/** Agents placed for each opening beat. Coordinates are world meters, clear of the walls. */
export const CUTSCENE_CAST: CastPose[][] = [
  [
    { index: 1, x: -44.5, z: 40, rot: Math.PI, speed: 1.4 },
    { index: 0, x: -40.2, z: 44, rot: Math.PI, speed: 1.2 },
    { index: 3, x: -36.5, z: 38, rot: Math.PI, speed: 1.3 },
  ],
  [
    { index: 1, x: -42.5, z: 16.2, rot: Math.PI, speed: 0.6 },
    { index: 0, x: -41.0, z: 18.4, rot: Math.PI, speed: 0 },
  ],
  [
    { index: 2, x: 44.2, z: 12.2, rot: Math.PI, speed: 0 },
    { index: 4, x: 49.2, z: 11.4, rot: Math.PI, speed: 0.5 },
  ],
  [
    { index: 3, x: 12.0, z: 38.4, rot: Math.PI, speed: 0.4 },
    { index: 5, x: 13.5, z: 41.0, rot: Math.PI, speed: 0 },
  ],
  [{ index: 2, x: -11.4, z: -45.2, rot: 0.2, speed: 0 }],
  [
    { index: 6, x: -41.8, z: -26.6, rot: 0.35, speed: 0 },
    { index: 7, x: -34.4, z: -27.0, rot: -0.35, speed: 0 },
  ],
];

const ROOM_WAYS: Array<[number, number]> = [
  [-3.2, -3.8],
  [2.8, -3.5],
  [-2.2, -4.2],
  [2.2, -4.0],
  [-3.4, -3.2],
  [3.0, -3.6],
];

/** Walk, idle, emote, and a trip to the south door. Clocks differ per agent. */
export function interiorPose(roomId: string, index: number, time: number, busy: boolean) {
  const crew = INTERIOR_CAST[roomId];
  const member = crew?.find((entry) => entry.index === index);
  const origin = ROOM_XZ[roomId];
  if (!member || !origin) return null;
  const [ox, oz] = origin;
  const span = (busy ? 12 : 18) * (0.82 + (index % 4) * 0.09);
  const phase = ((time + index * 2.6) % span) / span;
  const shift = (index % 3) * 0.35;
  const ways = ROOM_WAYS.map(([x, z], step) => [x + shift * (step % 2 === 0 ? 1 : -1), z] as [number, number]);
  const from = ways[index % ways.length];
  const to = ways[(index + 2) % ways.length];
  const door: [number, number] = [((index % 3) - 1) * 1.8, -3.2];
  const lane = crew.findIndex((entry) => entry.index === index);
  const clear = (x: number, z: number) => {
    let lx = x - ox;
    let lz = z - oz;
    if (roomId === "skin-forge") {
      lx = [-1.35, 0, 1.35][Math.max(0, lane)] ?? 0;
      lz = -2.5;
    } else {
      if (lz > -3.2) lz = -3.2;
      if (Math.abs(lx) < 1.5) lx = lx < 0 ? -2.1 : 2.1;
      lx = Math.min(4.1, Math.max(-4.1, lx));
      lz = Math.max(-4.4, lz);
    }
    return { x: ox + lx, z: oz + lz };
  };
  const walk = (a: [number, number], b: [number, number], u: number) => {
    const clamped = Math.min(1, Math.max(0, u));
    const posed = clear(ox + a[0] + (b[0] - a[0]) * clamped, oz + a[1] + (b[1] - a[1]) * clamped);
    return {
      x: posed.x,
      z: posed.z,
      rot: Math.atan2(b[0] - a[0], b[1] - a[1]),
      clip: "walk" as const,
      visible: true,
    };
  };
  if (phase < 0.42) {
    const u = phase / 0.42;
    const ping = u < 0.5 ? u * 2 : (1 - u) * 2;
    return walk(from, to, ping);
  }
  if (phase < 0.58) {
    const posed = clear(ox + to[0], oz + to[1]);
    return { x: posed.x, z: posed.z, rot: Math.PI, clip: "idle" as const, visible: true };
  }
  if (phase < 0.72) {
    const posed = clear(ox + to[0], oz + to[1]);
    return { x: posed.x, z: posed.z, rot: Math.PI, clip: "celebrate" as const, visible: true };
  }
  if (phase < 0.84) {
    const posed = clear(ox + from[0] * 0.4, oz - 2.2);
    return { x: posed.x, z: posed.z, rot: 0, clip: "idle" as const, visible: true };
  }
  const doorU = (phase - 0.84) / 0.16;
  if (doorU < 0.45) return walk(from, door, doorU / 0.45);
  if (doorU < 0.62) return { x: ox + door[0], z: oz + door[1], rot: 0, clip: "walk" as const, visible: false };
  return walk(door, to, (doorU - 0.62) / 0.38);
}

/** Crew standing inside a room. Coordinates are world meters, clear of the entry camera. */
export const INTERIOR_CAST: Record<string, CastPose[]> = {
  // World meters. Flank the boards; the frame audit rejects anyone on a screen or under 3 m.
  "vault-quarter": [
    { index: 0, x: -43.3, z: 13.1, rot: Math.PI, speed: 0, clip: "idle" },
    { index: 9, x: -41.85, z: 13.65, rot: Math.PI, speed: 0, clip: "celebrate" },
    { index: 1, x: -40.6, z: 13.0, rot: Math.PI, speed: 1.1, clip: "walk" },
  ],
  "trading-village": [
    { index: 2, x: 41.6, z: 10.8, rot: Math.PI, speed: 1.1, clip: "walk" },
    { index: 5, x: 44.8, z: 11.6, rot: Math.PI, speed: 0, clip: "idle" },
    { index: 8, x: 47.6, z: 11.4, rot: Math.PI, speed: 0, clip: "celebrate" },
  ],
  "war-room": [
    { index: 3, x: 9.5, z: 37.55, rot: 0, speed: 1.0, clip: "walk" },
    { index: 5, x: 11.15, z: 38.15, rot: 0.15, speed: 0, clip: "idle" },
    { index: 7, x: 14.7, z: 37.45, rot: -0.2, speed: 0, clip: "celebrate" },
  ],
  "skin-forge": [
    { index: 4, x: -39.35, z: -26.5, rot: Math.PI, speed: 0, clip: "idle" },
    { index: 7, x: -38, z: -27.3, rot: Math.PI, speed: 1.1, clip: "walk" },
    { index: 6, x: -36.65, z: -26.5, rot: Math.PI, speed: 0, clip: "celebrate" },
  ],
  "cup-stadium": [
    { index: 2, x: -10.05, z: -43.7, rot: 0.2, speed: 0, clip: "idle" },
    { index: 6, x: -5.95, z: -43.55, rot: -0.2, speed: 1.1, clip: "walk" },
    { index: 8, x: -7.95, z: -44.45, rot: 0.05, speed: 0, clip: "celebrate" },
  ],
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
