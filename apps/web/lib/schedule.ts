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

export function dailySlot(now: number): Slot {
  const today = dayStart(now) + OPEN;
  const start = now < today + DAILY_LEN ? today : today + DAY;
  return { kind: "daily", start, end: start + DAILY_LEN };
}

export function weeklySlot(now: number): Slot {
  const sinceMonday = (weekday(now) + 6) % 7;
  const monday = dayStart(now) - sinceMonday * DAY;
  const thisStart = monday + OPEN;
  const start = now < thisStart + WEEKLY_LEN ? thisStart : thisStart + 7 * DAY;
  return { kind: "weekly", start, end: start + WEEKLY_LEN };
}

export function seasonSlot(): Slot {
  const startDay = dayStart(SEASON_START);
  const delta = (6 + 7 - weekday(startDay)) % 7;
  const start = startDay + delta * DAY + OPEN;
  return { kind: "seasonal", start, end: start + SEASONAL_LEN };
}

function seasonal(now: number): Slot | null {
  if (now >= SEASON_END) return null;
  const slot = seasonSlot();
  if (now >= slot.end) return null;
  return slot;
}

export function nextHeist(now: number): Slot {
  let best = dailySlot(now);
  const weekly = weeklySlot(now);
  if (weekly.start < best.start) best = weekly;
  const season = seasonal(now);
  if (season && season.start < best.start) best = season;
  return best;
}

export function countdownSecs(now: number) {
  const slot = nextHeist(now);
  return now >= slot.start ? 0 : slot.start - now;
}

/** LIVE counts down the open window. NEXT counts down to the following slot. */
export function heistCountdown(now: number) {
  const slot = nextHeist(now);
  if (now >= slot.start && now < slot.end) return { live: true, secs: slot.end - now };
  return { live: false, secs: Math.max(0, slot.start - now) };
}

export function heistLive(now: number) {
  const slot = nextHeist(now);
  return now >= slot.start && now < slot.end;
}

export function openSlots(now: number): Slot[] {
  const slots = [dailySlot(now), weeklySlot(now)];
  const season = seasonal(now);
  if (season) slots.push(season);
  return slots.filter((slot) => now >= slot.start && now < slot.end);
}

export function slotPhase(now: number, slot: Slot): "live" | "upcoming" | "ended" {
  if (now < slot.start) return "upcoming";
  if (now < slot.end) return "live";
  return "ended";
}

export type EventPhase = "example" | "upcoming" | "live" | "ended";

export function eventPhase(now: number, start: number, end: number, exampleOnly: boolean): EventPhase {
  if (exampleOnly) return "example";
  if (now < start) return "upcoming";
  if (now < end) return "live";
  return "ended";
}

export function eventStatusLabel(phase: EventPhase) {
  if (phase === "example") return "Example event";
  if (phase === "upcoming") return "Upcoming";
  if (phase === "live") return "Live";
  return "Ended";
}

/**
 * Offline copy of `city_roster` in fe2vi-core. Used only when the API does not answer.
 * The live chip and the map both use this cast: ten agents, not the six score-tape ids.
 */
export const FIXTURE_AGENTS = [
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
];

export function formatClock(secs: number) {
  const whole = Math.max(0, Math.floor(secs));
  const hours = Math.floor(whole / 3600);
  const minutes = Math.floor((whole % 3600) / 60);
  const seconds = whole % 60;
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${pad(hours)}:${pad(minutes)}:${pad(seconds)}`;
}

function hourMinute(unix: number, timeZone: string) {
  return new Intl.DateTimeFormat("en-US", {
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
    timeZone,
  }).format(unix * 1000);
}

export type WindowText = { utc: string; wat: string };

function utcDay(unix: number) {
  return new Intl.DateTimeFormat("en-CA", {
    timeZone: "UTC",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(unix * 1000);
}

function stamp(unix: number, timeZone: string) {
  return new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
    timeZone,
  }).format(unix * 1000);
}

/** UTC first. WAT is Africa/Lagos, UTC+1. A range that crosses midnight names both dates. */
export function slotWindow(start: number, end: number): WindowText {
  if (utcDay(start) === utcDay(end)) {
    const day = new Intl.DateTimeFormat("en-US", {
      month: "short",
      day: "numeric",
      timeZone: "UTC",
    }).format(start * 1000);
    return {
      utc: `${day} · ${hourMinute(start, "UTC")}–${hourMinute(end, "UTC")} UTC`,
      wat: `${hourMinute(start, "Africa/Lagos")}–${hourMinute(end, "Africa/Lagos")} WAT`,
    };
  }
  return {
    utc: `${stamp(start, "UTC")} – ${stamp(end, "UTC")} UTC`,
    wat: `${stamp(start, "Africa/Lagos")} – ${stamp(end, "Africa/Lagos")} WAT`,
  };
}

/** Next seasonal Saturday that has not ended. Bounds come from the API season. */
export function upcomingSeason(now: number, seasonStart: number, seasonEnd: number): Slot {
  const span = Math.max(seasonEnd - seasonStart, DAY);
  const open = 18 * 3600;
  const len = 3 * 3600;
  let origin = seasonStart;
  for (let step = 0; step < 16; step += 1) {
    const startDay = origin - (origin % DAY);
    const delta = (6 + 7 - ((Math.floor(startDay / DAY) + 4) % 7)) % 7;
    const start = startDay + delta * DAY + open;
    const end = start + len;
    if (now < end) return { kind: "seasonal", start, end };
    origin += span;
  }
  return { kind: "seasonal", start: origin, end: origin + len };
}

export function formatCountdown(secs: number) {
  const whole = Math.max(0, Math.floor(secs));
  const hours = Math.floor(whole / 3600);
  const minutes = Math.floor((whole % 3600) / 60);
  const seconds = whole % 60;
  const mm = String(minutes).padStart(2, "0");
  const ss = String(seconds).padStart(2, "0");
  if (hours > 0) return `${String(hours).padStart(2, "0")}:${mm}:${ss}`;
  return `${mm}:${ss}`;
}

export function localRange(start: number, end: number) {
  const text = slotWindow(start, end);
  return `${text.utc} · ${text.wat}`;
}

export function exampleHeadline(now: number, start: number, end: number) {
  const run = now < start ? "Upcoming" : now < end ? "Running" : "Ended";
  return `Example · ${run}`;
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
