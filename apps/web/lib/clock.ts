"use client";

import { useEffect, useState } from "react";
import { apiBase } from "@/lib/protocol";

type Sample = { serverNow: number; clientMs: number };

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
      sample = { serverNow: Math.floor(clientMs / 1000), clientMs };
      notify();
    }
    return Promise.resolve();
  }
  inflight = fetch(`${base}/v1/schedule`, { cache: "no-store" })
    .then((response) => (response.ok ? response.json() : Promise.reject(new Error("schedule"))))
    .then((body: { now?: number }) => {
      const clientMs = Date.now();
      const serverNow = typeof body.now === "number" ? body.now : Math.floor(clientMs / 1000);
      sample = { serverNow, clientMs };
      notify();
    })
    .catch(() => {
      if (!sample) {
        const clientMs = Date.now();
        sample = { serverNow: Math.floor(clientMs / 1000), clientMs };
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
