"use client";

import { Bot, Clock, Gamepad2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { loadHome, type HomeRelay } from "@/lib/api";
import { FIXTURE_AGENTS, formatClock, heistCountdown, openSlots } from "@/lib/schedule";

type HomeView = {
  source: "relay" | "fixture";
  note: string;
  agents: number;
  liveGames: number;
  secs: number;
};

function derive(now: number, agents: number, events: HomeRelay["events"], source: HomeView["source"], note: string): HomeView {
  const slots = openSlots(now).length;
  const partners = events.filter((event) => !event.exampleOnly && now >= event.start && now < event.end).length;
  return {
    source,
    note,
    agents,
    liveGames: slots + partners,
    secs: heistCountdown(now).secs,
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
        const note = loaded.relay.agentsFrom === "fixture" ? "Fixture. /v1/stats did not answer." : "";
        setView(derive(loaded.relay.now, loaded.relay.agents, loaded.relay.events, loaded.relay.agentsFrom, note));
      } else {
        relay.current = null;
        const note = loaded.reason === "unset" ? "Fixture. This build has no API URL." : "Fixture. The API did not answer.";
        setView(derive(Math.floor(Date.now() / 1000), FIXTURE_AGENTS.length, [], "fixture", note));
      }
    };
    void pull();
    const tick = window.setInterval(() => {
      const held = relay.current;
      const now = held ? held.now + Math.max(0, Math.floor((Date.now() - heldAt.current) / 1000)) : Math.floor(Date.now() / 1000);
      setView((current) => {
        if (!current) return current;
        const events = held ? held.events : [];
        const agents = held ? held.agents : FIXTURE_AGENTS.length;
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
    ) : (
      <span className="chip">
        <Clock aria-hidden="true" strokeWidth={1.75} />
        next heist in <span className="num">{formatClock(view.secs)}</span>
      </span>
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
          Fixture
        </span>
      ) : null}
    </p>
  );
}
