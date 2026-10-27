import { avatarSpec, displayName, type AvatarSpec } from "./avatar";

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
    station: "Vault",
    color: "#ff6b4a",
  },
  {
    id: "cup-stadium",
    name: "Cup Stadium",
    summary: "Trading Cup. A short public tape.",
    status: "live",
    href: "/games/trading-cup-template",
    station: "Tape",
    color: "#ffd166",
  },
  {
    id: "trading-village",
    name: "Trading Village",
    summary: "Open tape, rolling boards, and follow.",
    status: "live",
    href: "/village",
    station: "Ledger",
    color: "#2ec4b6",
  },
  {
    id: "war-room",
    name: "War Room",
    summary: "A breach challenge on this contract only.",
    status: "live",
    href: "/war-room",
    station: "Breach",
    color: "#ef476f",
  },
  {
    id: "hack-hall",
    name: "Hack Hall",
    summary: "Coming soon.",
    status: "soon",
    href: "",
    station: "Soon",
    color: "#c77dff",
  },
  {
    id: "oracle-plaza",
    name: "Oracle Plaza",
    summary: "Coming soon.",
    status: "soon",
    href: "",
    station: "Soon",
    color: "#8ecae6",
  },
  {
    id: "launch-pier",
    name: "Launch Pier",
    summary: "Coming soon.",
    status: "soon",
    href: "",
    station: "Soon",
    color: "#f4a261",
  },
  {
    id: "card-club",
    name: "Card Club",
    summary: "Coming soon.",
    status: "soon",
    href: "",
    station: "Soon",
    color: "#b5e48c",
  },
];

export const DISTRICTS: District[] = LAYOUT.map((district, index) => {
  const col = index % 4;
  const row = Math.floor(index / 4);
  return { ...district, x: (col - 1.5) * 7, z: (row - 0.5) * 8 };
});

export type CityAgent = {
  accountId: string;
  name: string;
  spec: AvatarSpec;
};

export const CITY_AGENTS: CityAgent[] = [
  "mason.village.fe2vi.testnet",
  "scout.vault.fe2vi.testnet",
  "tape.cup.fe2vi.testnet",
  "pike.war.fe2vi.testnet",
].map((accountId) => ({
  accountId,
  name: displayName(accountId),
  spec: avatarSpec(accountId),
}));

/** Room centers in a loop, so agents walk door to door. */
export const WALK_LOOP: Array<[number, number]> = [
  0, 1, 2, 3, 7, 6, 5, 4,
].map((index) => [DISTRICTS[index].x, DISTRICTS[index].z]);

export function placeAgent(index: number, time: number, paused: boolean) {
  const phase = index * 1.7;
  const home = WALK_LOOP[index % WALK_LOOP.length];
  if (paused) {
    return { x: home[0], y: 0.78, z: home[1], rot: 0 };
  }
  const travel = time * 0.28 + phase;
  const span = WALK_LOOP.length;
  const wrapped = ((travel % span) + span) % span;
  const slot = Math.floor(wrapped) % span;
  const t = wrapped - Math.floor(wrapped);
  const smooth = t * t * (3 - 2 * t);
  const from = WALK_LOOP[slot];
  const to = WALK_LOOP[(slot + 1) % span];
  const bob = Math.sin(time * 7 + phase) * 0.07;
  const dx = to[0] - from[0];
  const dz = to[1] - from[1];
  return {
    x: from[0] + dx * smooth,
    y: 0.78 + bob,
    z: from[1] + dz * smooth,
    rot: Math.atan2(dx, dz),
  };
}
