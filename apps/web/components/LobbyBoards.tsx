"use client";

import { Calendar, Clock, Trophy } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { loadLobby, type LobbyRelay } from "@/lib/api";
import {
  CHALLENGES,
  EXAMPLE_EVENT,
  exampleEventLive,
  formatCountdown,
  heistCountdown,
  heistRows,
  localRange,
  mockQuotes,
  nextHeist,
  SEASON_END,
  SEASON_START,
  type HeistWindow,
} from "@/lib/schedule";

type LobbyView = {
  source: "relay" | "fixture";
  note: string;
  now: number;
  live: boolean;
  secs: number;
  slotKind: string;
  slotStart: number;
  slotEnd: number;
  seasonStart: number;
  seasonEnd: number;
  heist: Record<HeistWindow, { name: string; points: number }[]>;
  quotes: { symbol: string; kind: string; mark: number; fundingBps: number; pnlMicros: number }[];
  challenges: { id: string; category: string; title: string; points: number }[];
  events: { id: string; partner: string; room: string; prize: string; exampleOnly: boolean; live: boolean }[];
};

const ROOM_LABEL: Record<string, string> = {
  "vault-quarter": "Vault",
  "cup-stadium": "Stadium",
  "trading-market": "Trading floor",
  "war-room": "War room",
  "skin-forge": "Lobby",
};

function clockOf(now: number, start: number, end: number) {
  const live = now >= start && now < end;
  return { live, secs: live ? end - now : Math.max(0, start - now) };
}

function fromRelay(relay: LobbyRelay, now: number): LobbyView {
  const clock = clockOf(now, relay.slot.start, relay.slot.end);
  return {
    source: "relay",
    note: "Relay. Testnet and mock mode.",
    now,
    live: clock.live,
    secs: clock.secs,
    slotKind: relay.slot.kind,
    slotStart: relay.slot.start,
    slotEnd: relay.slot.end,
    seasonStart: relay.seasonStart,
    seasonEnd: relay.seasonEnd,
    heist: {
      today: relay.heist.today.map(({ name, points }) => ({ name, points })),
      week: relay.heist.week.map(({ name, points }) => ({ name, points })),
      season: relay.heist.season.map(({ name, points }) => ({ name, points })),
    },
    quotes: relay.quotes.map((row) => ({
      symbol: row.symbol,
      kind: row.kind,
      mark: row.markMicros,
      fundingBps: row.fundingBps,
      pnlMicros: row.pnlMicros,
    })),
    challenges: relay.challenges,
    events: relay.events,
  };
}

function fromFixture(now: number, reason: "unset" | "unreachable"): LobbyView {
  const slot = nextHeist(now);
  const clock = heistCountdown(now);
  const live = exampleEventLive(now);
  return {
    source: "fixture",
    note: reason === "unset" ? "Fixture. This build has no API URL." : "Fixture. The API did not answer.",
    now,
    live: clock.live,
    secs: clock.secs,
    slotKind: slot.kind,
    slotStart: slot.start,
    slotEnd: slot.end,
    seasonStart: SEASON_START,
    seasonEnd: SEASON_END,
    heist: {
      today: heistRows("today", now),
      week: heistRows("week", now),
      season: heistRows("season", now),
    },
    quotes: mockQuotes(now).map((row) => ({
      symbol: row.symbol,
      kind: row.kind,
      mark: row.mark,
      fundingBps: row.fundingBps,
      pnlMicros: row.pnlMicros,
    })),
    challenges: CHALLENGES,
    events: [
      {
        id: "example-week",
        partner: EXAMPLE_EVENT.partner,
        room: EXAMPLE_EVENT.room,
        prize: EXAMPLE_EVENT.prize,
        exampleOnly: true,
        live,
      },
    ],
  };
}

function retick(view: LobbyView, now: number): LobbyView {
  if (view.source === "fixture") return fromFixture(now, view.note.includes("no API") ? "unset" : "unreachable");
  const clock = clockOf(now, view.slotStart, view.slotEnd);
  return { ...view, now, live: clock.live, secs: clock.secs };
}

export function LobbyBoards() {
  const [view, setView] = useState<LobbyView | null>(null);
  const [windowName, setWindowName] = useState<HeistWindow>("today");
  const relay = useRef<LobbyRelay | null>(null);
  const heldAt = useRef(0);

  useEffect(() => {
    let dead = false;
    let generation = 0;
    const pull = async () => {
      const ticket = ++generation;
      const loaded = await loadLobby();
      if (dead || ticket !== generation) return;
      if (loaded.ok) {
        relay.current = loaded.relay;
        heldAt.current = Date.now();
        setView(fromRelay(loaded.relay, loaded.relay.now));
      } else {
        relay.current = null;
        setView(fromFixture(Math.floor(Date.now() / 1000), loaded.reason));
      }
    };
    void pull();
    const tick = window.setInterval(() => {
      const held = relay.current;
      const now = held
        ? held.now + Math.max(0, Math.floor((Date.now() - heldAt.current) / 1000))
        : Math.floor(Date.now() / 1000);
      setView((current) => (current ? retick(current, now) : current));
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
      <>
        <p className="chip">
          <Clock size={14} aria-hidden="true" />
          Heist clock
        </p>
        <ul className="schedule-list">
          <li>Season window is UTC.</li>
          <li>Daily 18:00 UTC, 60 minutes. Weekly Monday 18:00 UTC, 120 minutes.</li>
          <li>The next slot follows the UTC clock.</li>
        </ul>
        <section className="board">
          <p className="muted">Scores follow the UTC clock.</p>
        </section>
        <section className="board">
          <p className="muted">Mock tape. No order is sent. No key is used.</p>
        </section>
        <section className="board">
          <p className="muted">Challenges and events follow the UTC clock.</p>
        </section>
      </>
    );
  }

  const label = view.live ? `LIVE ${formatCountdown(view.secs)}` : `Next heist in ${formatCountdown(view.secs)}`;
  const rows = view.heist[windowName];
  const liveEvent = view.events.find((event) => event.live);

  return (
    <>
      <p className="meta">{view.note}</p>
      <p className="chip">
        <Clock size={14} aria-hidden="true" />
        {label} · {view.slotKind} · {localRange(view.slotStart, view.slotEnd)}
      </p>
      {liveEvent ? (
        <p className="banner">
          Event live · {liveEvent.partner}
          {liveEvent.exampleOnly ? " · example only" : ""} · {ROOM_LABEL[liveEvent.room] ?? liveEvent.room} · {liveEvent.prize}
        </p>
      ) : null}
      <ul className="schedule-list">
        <li>Season opens {localRange(view.seasonStart, view.seasonEnd)}</li>
        <li>Daily 18:00 UTC, 60 minutes. Weekly Monday 18:00 UTC, 120 minutes.</li>
        <li>
          Next {view.slotKind} slot {localRange(view.slotStart, view.slotEnd)}
        </li>
      </ul>
      <section className="board">
        <div className="tabs">
          {(["today", "week", "season"] as const).map((id) => (
            <button key={id} type="button" onClick={() => setWindowName(id)} aria-pressed={windowName === id}>
              {id === "today" ? <Clock size={16} aria-hidden="true" /> : id === "week" ? <Calendar size={16} aria-hidden="true" /> : <Trophy size={16} aria-hidden="true" />}
              {id}
            </button>
          ))}
        </div>
        <ol>
          {rows.length === 0 ? (
            <li>No scores in this window.</li>
          ) : (
            rows.map((row) => (
              <li key={row.name}>
                {row.name} {row.points}
              </li>
            ))
          )}
        </ol>
      </section>
      <section className="board">
        <ul>
          {view.quotes.map((row) => (
            <li key={row.symbol}>
              {row.kind} {row.symbol} {(row.mark / 1_000_000).toFixed(2)} · funding {row.fundingBps} bps · pnl {(row.pnlMicros / 1_000_000).toFixed(2)}
            </li>
          ))}
        </ul>
        <p className="muted">Mock tape. No order is sent. No key is used.</p>
      </section>
      <section className="board">
        <h2>Challenges</h2>
        <ul>
          {view.challenges.map((row) => (
            <li key={row.id}>
              {row.category} · {row.title} · {row.points}
            </li>
          ))}
        </ul>
      </section>
      <section className="board">
        <h2>Events</h2>
        <ul>
          {view.events.map((event) => (
            <li key={event.id}>
              {event.partner}
              {event.exampleOnly ? " · example only" : ""} · {ROOM_LABEL[event.room] ?? event.room} · {event.live ? "live" : "scheduled"} · {event.prize}
            </li>
          ))}
        </ul>
      </section>
    </>
  );
}
