export const ONE_NEAR = 10n ** 24n;

export const DEFAULT_CAPS = {
  maxPlayers: 8,
  maxDurationSeconds: 24 * 60 * 60,
  maxRoundCount: 8,
  maxPotNear: 50,
  maxPerAgentNear: 5,
  maxPerActionNear: 1,
};

export type Rules = {
  template_id: string;
  max_players: number;
  duration_seconds: number;
  round_count: number;
  pot_yocto: string;
  per_agent_cap_yocto: string;
  per_action_cap_yocto: string;
  entry_yocto: string;
};

export type Trust = {
  hash_pinned: boolean;
  within_platform_caps: boolean;
  known_template: boolean;
  delisted: boolean;
  creator_text: "untrusted";
  network: "testnet";
  relay: "unverified" | "fixture";
};

export type GameCard = {
  id: string;
  creator: string;
  rules: Rules;
  game_md_hash: string;
  delisted: boolean;
  verified_events?: number;
  trust: Trust;
};

export type AgentCard = {
  account_id: string;
  revoked: boolean;
  allowed_methods: string[];
  platform_caps: {
    max_players: number;
    max_duration_seconds: number;
    max_round_count: number;
    max_pot_yocto: string;
    max_per_agent_yocto: string;
    max_per_action_yocto: string;
  };
  network: "testnet";
  relay: string;
  note: string;
};

export type FeedEvent = {
  id: number;
  game_id: string;
  kind: string;
  summary: string;
  verified?: boolean;
  source?: string;
};

export function canonicalRules(rules: Rules): string {
  return JSON.stringify({
    template_id: rules.template_id,
    max_players: rules.max_players,
    duration_seconds: rules.duration_seconds,
    round_count: rules.round_count,
    pot_yocto: rules.pot_yocto,
    per_agent_cap_yocto: rules.per_agent_cap_yocto,
    per_action_cap_yocto: rules.per_action_cap_yocto,
    entry_yocto: rules.entry_yocto,
  });
}

export function nearToYocto(input: string): string | null {
  const trimmed = input.trim();
  if (!/^\d+(\.\d+)?$/.test(trimmed)) return null;
  const [whole, frac = ""] = trimmed.split(".");
  if (frac.length > 24) return null;
  const digits = `${whole}${frac.padEnd(24, "0")}`.replace(/^0+(?=\d)/, "");
  return digits === "" ? "0" : digits;
}

export function yoctoToNear(yocto: string): string {
  const padded = yocto.padStart(25, "0");
  const whole = padded.slice(0, -24).replace(/^0+(?=\d)/, "");
  const frac = padded.slice(-24).replace(/0+$/, "");
  return frac ? `${whole}.${frac}` : whole;
}

export async function sha256Hex(text: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

export function apiBase(): string {
  return process.env.NEXT_PUBLIC_FE2VI_API_URL?.replace(/\/$/, "") ?? "";
}

export function withinDefaultCaps(rules: Rules): string | null {
  if (rules.template_id !== "heist-night" && rules.template_id !== "trading-cup") {
    return "Unknown game.";
  }
  if (rules.max_players < 1 || rules.max_players > DEFAULT_CAPS.maxPlayers) return "Players exceed the platform cap.";
  if (rules.duration_seconds < 1 || rules.duration_seconds > DEFAULT_CAPS.maxDurationSeconds) {
    return "Duration exceeds the platform cap.";
  }
  if (rules.round_count < 1 || rules.round_count > DEFAULT_CAPS.maxRoundCount) return "Rounds exceed the platform cap.";
  const pot = BigInt(rules.pot_yocto);
  const agent = BigInt(rules.per_agent_cap_yocto);
  const action = BigInt(rules.per_action_cap_yocto);
  const entry = BigInt(rules.entry_yocto);
  if (pot <= 0n || pot > BigInt(DEFAULT_CAPS.maxPotNear) * ONE_NEAR) return "Pot exceeds the platform cap.";
  if (agent <= 0n || agent > BigInt(DEFAULT_CAPS.maxPerAgentNear) * ONE_NEAR) return "Per-agent cap exceeds the platform cap.";
  if (action <= 0n || action > BigInt(DEFAULT_CAPS.maxPerActionNear) * ONE_NEAR) return "Per-action cap exceeds the platform cap.";
  if (action > agent) return "Per-action cap exceeds the per-agent cap.";
  if (entry > agent) return "Entry exceeds the per-agent cap.";
  return null;
}
