/** Spectator clock. The numbers match fe2vi-core schedule and mock quotes. */

import { simulatedPriceMicros } from "./tape";

export const SEASON_START = 1_790_812_800;
export const SEASON_END = 1_798_761_600;
const DAY = 86_400;
const OPEN = 18 * 3600;
const DAILY_LEN = 3600;
const WEEKLY_LEN = 2 * 3600;
const SEASONAL_LEN = 3 * 3600;

export type SlotKind = "daily" | "weekly" | "seasonal";
export type Slot = { kind: SlotKind; start: number; end: number };
export type HeistWindow = "today" | "week" | "season";

function weekday(unix: number) {
  return (Math.floor(unix / DAY) + 4) % 7;
}

function dayStart(unix: number) {
  return unix - (unix % DAY);
}

function nextDaily(now: number): Slot {
  const today = dayStart(now) + OPEN;
  const start = now < today + DAILY_LEN ? today : today + DAY;
  return { kind: "daily", start, end: start + DAILY_LEN };
}

function nextWeekly(now: number): Slot {
  const sinceMonday = (weekday(now) + 6) % 7;
  const monday = dayStart(now) - sinceMonday * DAY;
  const thisStart = monday + OPEN;
  const start = now < thisStart + WEEKLY_LEN ? thisStart : thisStart + 7 * DAY;
  return { kind: "weekly", start, end: start + WEEKLY_LEN };
}

function seasonal(now: number): Slot | null {
  if (now >= SEASON_END) return null;
  const startDay = dayStart(SEASON_START);
  const delta = (6 + 7 - weekday(startDay)) % 7;
  const start = startDay + delta * DAY + OPEN;
  const end = start + SEASONAL_LEN;
  if (now >= end) return null;
  return { kind: "seasonal", start, end };
}

export function nextHeist(now: number): Slot {
  let best = nextDaily(now);
  const weekly = nextWeekly(now);
  if (weekly.start < best.start) best = weekly;
  const season = seasonal(now);
  if (season && season.start < best.start) best = season;
  return best;
}

export function countdownSecs(now: number) {
  const slot = nextHeist(now);
  return now >= slot.start ? 0 : slot.start - now;
}

export function heistLive(now: number) {
  const slot = nextHeist(now);
  return now >= slot.start && now < slot.end;
}

export function formatCountdown(secs: number) {
  const hours = Math.floor(secs / 3600);
  const minutes = Math.floor((secs % 3600) / 60);
  return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}`;
}

export function localRange(start: number, end: number) {
  const fmt = new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
  return `${fmt.format(start * 1000)} to ${fmt.format(end * 1000)}`;
}

const SCORES = [
  { name: "Mason", points: 12, unix: 1_793_116_800 - 3600 },
  { name: "Scout", points: 9, unix: 1_793_116_800 - 1800 },
  { name: "Pike", points: 7, unix: 1_793_116_800 - DAY },
  { name: "Tape", points: 15, unix: SEASON_START + DAY },
  { name: "Quill", points: 5, unix: SEASON_START + 2 * DAY },
];

export function heistRows(window: HeistWindow, now: number) {
  const start = window === "today" ? dayStart(now) : window === "week" ? dayStart(now) - ((weekday(now) + 6) % 7) * DAY : SEASON_START;
  return SCORES.filter((row) => row.unix >= start && row.unix <= now).sort((a, b) => b.points - a.points || a.name.localeCompare(b.name));
}

export function mockQuotes(now: number) {
  return [
    ["PERP-NEAR", "perp"],
    ["PERP-BTC", "perp"],
    ["STK-IRON", "stock"],
    ["STK-VAULT", "stock"],
  ].map(([symbol, kind], index) => {
    const mark = simulatedPriceMicros(symbol, now);
    const previous = simulatedPriceMicros(symbol, now - 3600);
    return { symbol, kind, mark, fundingBps: 2 + index, pnlMicros: mark - previous };
  });
}

export const CHALLENGES = [
  { id: "referee", title: "Referee", category: "contract", points: 100 },
  { id: "sandbox-page", title: "Sandbox page", category: "web2", points: 150 },
  { id: "hash-puzzle", title: "Hash puzzle", category: "crypto", points: 80 },
];

export const EXAMPLE_EVENT = {
  partner: "Example partner",
  room: "vault-quarter",
  roomLabel: "Vault",
  prize: "Example prize text. Not a real partner.",
  rules: "Example rules. Agents still use the on-chain caps.",
  start: SEASON_START,
  end: SEASON_START + 40 * DAY,
};

export function exampleEventLive(now: number) {
  return now >= EXAMPLE_EVENT.start && now < EXAMPLE_EVENT.end;
}
