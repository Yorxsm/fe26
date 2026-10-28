"use client";

import { useEffect, useState } from "react";
import { apiBase } from "@/lib/protocol";

type RoomSlot = { live: boolean; start: number; end: number; kind: string };
type Sample = {
  serverNow: number;
  clientMs: number;
  live: boolean;
  start: number;
  end: number;
  rooms: Record<string, RoomSlot>;
};

function readRooms(value: unknown): Record<string, RoomSlot> {
  const rooms: Record<string, RoomSlot> = {};
  if (!Array.isArray(value)) return rooms;
  for (const item of value) {
    if (!item || typeof item !== "object") continue;
    const row = item as { room?: unknown; kind?: unknown; start_unix?: unknown; end_unix?: unknown };
    if (typeof row.room !== "string" || typeof row.start_unix !== "number" || typeof row.end_unix !== "number") continue;
    rooms[row.room] = {
      live: false,
      start: row.start_unix,
      end: row.end_unix,
      kind: typeof row.kind === "string" ? row.kind : "",
    };
  }
  return rooms;
}

let sample: Sample | null = null;
let inflight: Promise<void> | null = null;
const listeners = new Set<() => void>();

function notify() {
  listeners.forEach((listener) => listener());
}

/** One server clock for every countdown. Falls back to the browser only if the schedule does not answer. */
export function syncHeistClock() {
  if (inflight) return inflight;
  const base = apiBase();
    if (!base) {
    if (!sample) {
      const clientMs = Date.now();
      const serverNow = Math.floor(clientMs / 1000);
      sample = { serverNow, clientMs, live: false, start: serverNow, end: serverNow, rooms: {} };
      notify();
    }
    return Promise.resolve();
  }
  inflight = fetch(`${base}/v1/schedule`, { cache: "no-store" })
    .then((response) => (response.ok ? response.json() : Promise.reject(new Error("schedule"))))
    .then((body: { now?: number; live?: boolean; next?: { start_unix?: number; end_unix?: number }; rooms?: unknown }) => {
      const clientMs = Date.now();
      const serverNow = typeof body.now === "number" ? body.now : Math.floor(clientMs / 1000);
      const start = typeof body.next?.start_unix === "number" ? body.next.start_unix : serverNow;
      const end = typeof body.next?.end_unix === "number" ? body.next.end_unix : start;
      sample = { serverNow, clientMs, live: Boolean(body.live), start, end, rooms: readRooms(body.rooms) };
      notify();
    })
    .catch(() => {
      if (!sample) {
        const clientMs = Date.now();
        const serverNow = Math.floor(clientMs / 1000);
        sample = { serverNow, clientMs, live: false, start: serverNow, end: serverNow, rooms: {} };
        notify();
      }
    })
    .finally(() => {
      inflight = null;
    });
  return inflight;
}

export function heistNow() {
  if (!sample) return null;
  return sample.serverNow + Math.max(0, Math.floor((Date.now() - sample.clientMs) / 1000));
}

/** Live flag and remaining seconds from the schedule response, not a fixed label. */
export function scheduleClock() {
  if (!sample) return null;
  const now = sample.serverNow + Math.max(0, Math.floor((Date.now() - sample.clientMs) / 1000));
  const live = sample.live && now < sample.end;
  const secs = live ? Math.max(0, sample.end - now) : Math.max(0, sample.start - now);
  return { live, secs };
}

export function roomClock(room: string) {
  const slot = sample?.rooms[room];
  const now = heistNow();
  if (!slot || now == null) return null;
  const live = now >= slot.start && now < slot.end;
  const secs = live ? Math.max(0, slot.end - now) : Math.max(0, slot.start - now);
  return { live, secs, kind: slot.kind };
}

export function useRoomClocks() {
  const [clocks, setClocks] = useState<Record<string, ReturnType<typeof roomClock>>>({});
  useEffect(() => {
    const pull = () => {
      const next: Record<string, ReturnType<typeof roomClock>> = {};
      for (const room of ["vault", "cup", "village", "war"]) next[room] = roomClock(room);
      setClocks(next);
    };
    listeners.add(pull);
    void syncHeistClock().then(pull);
    const tick = window.setInterval(pull, 1000);
    const refresh = window.setInterval(() => {
      void syncHeistClock();
    }, 30_000);
    return () => {
      listeners.delete(pull);
      window.clearInterval(tick);
      window.clearInterval(refresh);
    };
  }, []);
  return clocks;
}

export function useScheduleClock() {
  const [clock, setClock] = useState<ReturnType<typeof scheduleClock>>(null);
  useEffect(() => {
    const pull = () => setClock(scheduleClock());
    listeners.add(pull);
    void syncHeistClock().then(pull);
    const tick = window.setInterval(pull, 1000);
    const refresh = window.setInterval(() => {
      void syncHeistClock();
    }, 30_000);
    return () => {
      listeners.delete(pull);
      window.clearInterval(tick);
      window.clearInterval(refresh);
    };
  }, []);
  return clock;
}

export function useHeistNow() {
  const [now, setNow] = useState<number | null>(null);
  useEffect(() => {
    const pull = () => setNow(heistNow());
    listeners.add(pull);
    void syncHeistClock().then(pull);
    const tick = window.setInterval(pull, 1000);
    const refresh = window.setInterval(() => {
      void syncHeistClock();
    }, 30_000);
    return () => {
      listeners.delete(pull);
      window.clearInterval(tick);
      window.clearInterval(refresh);
    };
  }, []);
  return now;
}
