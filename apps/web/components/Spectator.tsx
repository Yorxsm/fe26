"use client";

import { useEffect, useState } from "react";
import { fixtureFeed } from "@/lib/api";
import { apiBase, type FeedEvent } from "@/lib/protocol";

export function Spectator({ gameId, source }: { gameId: string; source: "relay" | "fixture" }) {
  const [events, setEvents] = useState<FeedEvent[]>([]);
  const [status, setStatus] = useState("connecting");

  useEffect(() => {
    const base = apiBase();
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (!base || source === "fixture") {
      const script = fixtureFeed(gameId);
      setStatus(source === "fixture" ? "fixture replay" : "no relay configured");
      if (script.length === 0) {
        setEvents([]);
        return;
      }
      if (reduced) {
        setEvents(script);
        return;
      }
      let index = 0;
      setEvents([script[0]]);
      const timer = window.setInterval(() => {
        index += 1;
        if (index >= script.length) {
          window.clearInterval(timer);
          return;
        }
        setEvents(script.slice(0, index + 1));
      }, 900);
      return () => window.clearInterval(timer);
    }

    let socket: WebSocket | null = null;
    let stopped = false;
    const wsBase = base.replace(/^http/, "ws");
    try {
      socket = new WebSocket(`${wsBase}/v1/games/${encodeURIComponent(gameId)}/ws`);
      socket.onopen = () => setStatus("live socket");
      socket.onmessage = (message) => {
        const payload = JSON.parse(String(message.data)) as FeedEvent[] | FeedEvent;
        const next = Array.isArray(payload) ? payload : [payload];
        setEvents((current) => {
          const merged = [...current];
          for (const event of next) {
            if (!merged.some((item) => item.id === event.id)) merged.push(event);
          }
          return merged;
        });
      };
      socket.onerror = () => setStatus("socket error, polling");
    } catch {
      setStatus("polling");
    }

    const poll = window.setInterval(async () => {
      try {
        const response = await fetch(`${base}/v1/games/${encodeURIComponent(gameId)}/events`);
        if (!response.ok) return;
        const payload = (await response.json()) as FeedEvent[];
        if (!stopped) {
          setEvents(payload);
          if (!socket || socket.readyState !== WebSocket.OPEN) setStatus("polling");
        }
      } catch {
        if (!stopped) setStatus("relay unreachable");
      }
    }, 2000);

    return () => {
      stopped = true;
      window.clearInterval(poll);
      socket?.close();
    };
  }, [gameId, source]);

  return (
    <section>
      <div className="row">
        <h2>Play by play</h2>
        <span className="meta">{status}</span>
      </div>
      <div className="feed" aria-live="polite">
        {events.length === 0 && <div className="line">Waiting for the first event.</div>}
        {events.map((event) => (
          <div className={event.verified ? "line verified" : "line"} key={event.id}>
            <span className="meta">
              {event.kind} · {event.verified ? "verified" : event.source ?? "relay"}
            </span>{" "}
            {event.summary}
          </div>
        ))}
      </div>
    </section>
  );
}
