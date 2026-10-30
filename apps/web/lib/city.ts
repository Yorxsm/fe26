import { displayName } from "./avatar";
import { defaultSkin, type SkinTraits } from "./skin";

export type QualityTier = "low" | "medium" | "high";

/** Rough plot bounds used by the camera to stay out of buildings. */
export type DistrictFootprint = { hx: number; hz: number; height: number };

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
  footprint: DistrictFootprint;
};

const LAYOUT: Array<Omit<District, "x" | "z" | "footprint">> = [
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
];

/** glTF meters from apps/web/public/models/layout.json. Y is up, so z is the file's third value. */
export const ROOM_XZ: Record<string, [number, number]> = {
  "vault-quarter": [-42, 18],
  "cup-stadium": [-8, -42],
  "trading-village": [46, 14],
  "war-room": [12, 40],
  "skin-forge": [-38, -24],
};

/**
 * Approximate plot bounds. The room shell runs x ±7.2 and z ±6.2, so the
 * footprint adds a little clearance for the facade, stairs, and signage.
 */
export const DISTRICT_FOOTPRINT: Record<string, DistrictFootprint> = {
  "vault-quarter": { hx: 8.8, hz: 8.0, height: 13.5 },
  "cup-stadium": { hx: 10.2, hz: 9.0, height: 15.0 },
  "trading-village": { hx: 9.0, hz: 8.2, height: 14.0 },
  "war-room": { hx: 8.8, hz: 8.0, height: 13.5 },
  "skin-forge": { hx: 9.0, hz: 8.2, height: 12.5 },
};

/** South face of each building. Agents use this when a room opens or closes. */
export const BUILDING_DOOR: Record<string, [number, number]> = {
  "vault-quarter": [-42, 21.4],
  "cup-stadium": [-8, -38.6],
  "trading-village": [46, 17.4],
  "war-room": [12, 43.4],
  "skin-forge": [-38, -20.6],
};

const ROOF_NAME: Record<string, string> = {
  "vault-quarter": "VAULT",
  "cup-stadium": "STADIUM",
  "trading-village": "TRADING VILLAGE",
  "war-room": "WAR ROOM",
  "skin-forge": "LOBBY",
};

export function roofLabel(id: string) {
  return ROOF_NAME[id] ?? "";
}

let transitionMs = 0;

export function markRoomTransition() {
  transitionMs = performance.now();
}

export function transitionAge() {
  return (performance.now() - transitionMs) / 1000;
}

/** Walk between the lawn and the door. Hidden at the threshold. */
export function thresholdPose(roomId: string, index: number, entering: boolean) {
  const door = BUILDING_DOOR[roomId];
  const crew = INTERIOR_CAST[roomId];
  if (!door || !crew?.some((member) => member.index === index)) return null;
  const age = transitionAge();
  if (age > 1.15) return null;
  const lawn: [number, number] = [door[0], door[1] + 3.4];
  const from = entering ? lawn : door;
  const to = entering ? door : lawn;
  const u = Math.min(1, age / 1.15);
  return {
    x: from[0] + (to[0] - from[0]) * u,
    z: from[1] + (to[1] - from[1]) * u,
    rot: entering ? Math.PI : 0,
    visible: entering ? u < 0.9 : u > 0.08,
  };
}

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

/** Open floor, clear of the north boards (local z about -5.7) and the south door. */
const ROOM_ROUTES: Array<Array<[number, number]>> = [
  [[-3.2, 1.6], [2.4, 1.2], [2.8, -1.4], [-1.6, -2.2]],
  [[3.0, 0.4], [0.4, -2.4], [-2.8, -0.8], [-1.2, 2.2]],
  [[-0.2, 2.4], [3.2, -0.2], [0.8, -2.6], [-3.0, 0.6]],
];

const LOBBY_ROUTE: Array<[number, number]> = [
  [-2.2, -2.1],
  [0, -1.5],
  [2.2, -2.1],
  [0, -2.7],
];

/** Walk between stations, then stand and work. No looping emote on one spot. */
export function interiorPose(roomId: string, index: number, time: number, busy: boolean) {
  const crew = INTERIOR_CAST[roomId];
  const member = crew?.find((entry) => entry.index === index);
  const origin = ROOM_XZ[roomId];
  if (!member || !origin) return null;
  const route = roomId === "skin-forge" ? LOBBY_ROUTE : ROOM_ROUTES[index % ROOM_ROUTES.length];
  const leg = (busy ? 9 : 12) + (index % 3);
  const travel = (time * 0.85 + index * 4.2) % (route.length * leg);
  const step = Math.floor(travel / leg) % route.length;
  const along = (travel % leg) / leg;
  const from = route[step];
  const to = route[(step + 1) % route.length];
  const [ox, oz] = origin;
  const world = (point: [number, number]) => ({ x: ox + point[0], z: oz + point[1] });
  const rot = Math.atan2(to[0] - from[0], to[1] - from[1]);
  if (along < 0.7) {
    const t = along / 0.7;
    const a = world(from);
    const b = world(to);
    return { x: a.x + (b.x - a.x) * t, z: a.z + (b.z - a.z) * t, rot, clip: "walk" as const, visible: true };
  }
  const stood = world(to);
  return { x: stood.x, z: stood.z, rot, clip: "idle" as const, visible: true };
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
  const footprint = DISTRICT_FOOTPRINT[district.id] ?? { hx: 9, hz: 8, height: 14 };
  return { ...district, x, z, footprint };
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
