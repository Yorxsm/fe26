"use client";

import { Bot, Clock, Gamepad2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { loadHome, type HomeRelay } from "@/lib/api";
import { formatClock, heistCountdown, openSlots } from "@/lib/schedule";

type HomeView = {
  source: "relay" | "unavailable";
  note: string;
  agents: number;
  liveGames: number;
  secs: number;
};

function derive(now: number, agents: number, events: HomeRelay["events"], source: HomeView["source"], note: string): HomeView {
  const slots = source === "relay" ? openSlots(now).length : 0;
  const partners = source === "relay" ? events.filter((event) => !event.exampleOnly && now >= event.start && now < event.end).length : 0;
  return {
    source,
    note,
    agents,
    liveGames: slots + partners,
    secs: source === "relay" ? heistCountdown(now).secs : 0,
  };
}

export function HomeChips() {
  const [view, setView] = useState<HomeView | null>(null);
  const relay = useRef<HomeRelay | null>(null);
  const heldAt = useRef(0);

  useEffect(() => {
    let dead = false;
    let generation = 0;
    const pull = async () => {
      const ticket = ++generation;
      const loaded = await loadHome();
      if (dead || ticket !== generation) return;
      if (loaded.ok) {
        relay.current = loaded.relay;
        heldAt.current = Date.now();
        setView(derive(loaded.relay.now, loaded.relay.agents, loaded.relay.events, "relay", ""));
      } else {
        relay.current = null;
        const note = loaded.reason === "unset" ? "No API URL is set." : "The API did not answer.";
        setView(derive(Math.floor(Date.now() / 1000), 0, [], "unavailable", note));
      }
    };
    void pull();
    const tick = window.setInterval(() => {
      const held = relay.current;
      const now = held ? held.now + Math.max(0, Math.floor((Date.now() - heldAt.current) / 1000)) : Math.floor(Date.now() / 1000);
      setView((current) => {
        if (!current) return current;
        const events = held ? held.events : [];
        const agents = held ? held.agents : 0;
        return derive(now, agents, events, current.source, current.note);
      });
    }, 1000);
    const refresh = window.setInterval(() => {
      void pull();
    }, 30_000);
    return () => {
      dead = true;
      window.clearInterval(tick);
      window.clearInterval(refresh);
    };
  }, []);

  if (!view) {
    return (
      <p className="live-chips" aria-live="polite">
        <span className="chip">
          <Bot aria-hidden="true" strokeWidth={1.75} />
          Agents
        </span>
        <span className="chip">
          <Clock aria-hidden="true" strokeWidth={1.75} />
          Heist clock
        </span>
      </p>
    );
  }

  const games =
    view.liveGames > 0 ? (
      <span className="chip">
        <span className="live-dot" aria-hidden="true" />
        <Gamepad2 aria-hidden="true" strokeWidth={1.75} />
        <span className="num">{view.liveGames}</span>
        {view.liveGames === 1 ? "live game" : "live games"}
      </span>
    ) : view.source === "relay" ? (
      <span className="chip">
        <Clock aria-hidden="true" strokeWidth={1.75} />
        next heist in <span className="num">{formatClock(view.secs)}</span>
      </span>
    ) : (
      <span className="chip">No live activity</span>
    );

  return (
    <p className="live-chips" aria-live="polite">
      <span className="chip">
        <Bot aria-hidden="true" strokeWidth={1.75} />
        <span className="num">{view.agents}</span>
        agents
      </span>
      {games}
      {view.note ? (
        <span className="chip" title={view.note}>
          {view.note}
        </span>
      ) : null}
    </p>
  );
}
