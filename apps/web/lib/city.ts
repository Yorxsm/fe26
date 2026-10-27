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
    station: "Vault Room",
    color: "#ff6b4a",
  },
  {
    id: "cup-stadium",
    name: "Cup Stadium",
    summary: "Trading Cup. A short public tape.",
    status: "live",
    href: "/games/trading-cup-template",
    station: "Stadium Room",
    color: "#ffd166",
  },
  {
    id: "trading-village",
    name: "Trading Village",
    summary: "Open tape, rolling boards, and follow.",
    status: "live",
    href: "/village",
    station: "Trading Market 24/7 Open",
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
  "vault-quarter": [-12, 8],
  "cup-stadium": [-8, -12],
  "trading-village": [12, -8],
  "war-room": [8, 12],
  "skin-forge": [0, 0],
  "launch-pier": [1.6, -18.4],
  "card-club": [-1.6, 18.4],
  "oracle-plaza": [-18.4, -1.6],
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
].map((accountId) => ({
  accountId,
  name: displayName(accountId),
  skin: defaultSkin(accountId),
}));

/** Door-to-door loop around the concourse. Side rooms stay off the path. */
export const WALK_LOOP: Array<[number, number]> = [
  "vault-quarter",
  "skin-forge",
  "cup-stadium",
  "skin-forge",
  "trading-village",
  "skin-forge",
  "war-room",
].map((id) => {
  const room = DISTRICTS.find((district) => district.id === id);
  if (!room) return [0, 0];
  return [room.x, room.z];
});

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
