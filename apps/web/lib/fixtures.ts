import type { AgentCard, FeedEvent, GameCard } from "./protocol";

export const HEIST_PIN = "320421571d7cf9e178b0b5c0aa1dce8d3ea3f2dbf0bdc507849aaf70a121651d";

export const fixtureGame: GameCard = {
  id: "heist-night",
  creator: "heist.fe2vi.testnet",
  rules: {
    template_id: "heist-night",
    max_players: 6,
    duration_seconds: 3600,
    round_count: 4,
    pot_yocto: "10000000000000000000000000",
    per_agent_cap_yocto: "2000000000000000000000000",
    per_action_cap_yocto: "500000000000000000000000",
    entry_yocto: "0",
  },
  game_md_hash: HEIST_PIN,
  delisted: false,
  trust: {
    hash_pinned: true,
    within_platform_caps: true,
    known_template: true,
    delisted: false,
    creator_text: "untrusted",
    network: "testnet",
    relay: "fixture",
  },
};

const CUP_HASH = "993ccadefb348204b7edf422e2bd80c9cf587ea3e48911f12e7951490cda6428";

export const fixtureCup: GameCard = {
  ...fixtureGame,
  id: "trading-cup",
  creator: "cup.fe2vi.testnet",
  game_md_hash: CUP_HASH,
  rules: { ...fixtureGame.rules, template_id: "trading-cup" },
};

export const fixtureGames = [fixtureGame, fixtureCup];

export const fixtureAgents: AgentCard[] = [
  {
    account_id: "demo.registry.testnet",
    revoked: false,
    allowed_methods: ["act", "bid", "transfer_item"],
    platform_caps: {
      max_players: 8,
      max_duration_seconds: 86400,
      max_round_count: 8,
      max_pot_yocto: "50000000000000000000000000",
      max_per_agent_yocto: "5000000000000000000000000",
      max_per_action_yocto: "1000000000000000000000000",
    },
    network: "testnet",
    relay: "fixture",
    note: "Fixture agent. The scoped key cannot call resolve, delist, or set caps.",
  },
  {
    account_id: "revoked.registry.testnet",
    revoked: true,
    allowed_methods: ["act", "bid", "transfer_item"],
    platform_caps: {
      max_players: 8,
      max_duration_seconds: 86400,
      max_round_count: 8,
      max_pot_yocto: "50000000000000000000000000",
      max_per_agent_yocto: "5000000000000000000000000",
      max_per_action_yocto: "1000000000000000000000000",
    },
    network: "testnet",
    relay: "fixture",
    note: "Fixture. A revoked agent cannot act, bid, or transfer an item.",
  },
];

export const fixtureEvents: FeedEvent[] = [
  { id: 1, game_id: fixtureGame.id, kind: "heist_opened", summary: "Pot escrowed in the heist contract.", verified: false, source: "fixture" },
  { id: 2, game_id: fixtureGame.id, kind: "player_joined", summary: "demo.registry.testnet joined.", verified: false, source: "fixture" },
  { id: 3, game_id: fixtureGame.id, kind: "action_taken", summary: "demo played scout within the per-action cap.", verified: false, source: "fixture" },
  { id: 4, game_id: fixtureGame.id, kind: "bid_placed", summary: "A bid for intel was reserved against the agent cap.", verified: false, source: "fixture" },
];

export function fixtureAgent(id: string): AgentCard | undefined {
  return fixtureAgents.find((agent) => agent.account_id === id);
}
