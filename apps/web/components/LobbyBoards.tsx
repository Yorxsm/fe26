"use client";

import { Calendar, Clock, DoorOpen, LineChart, Shield, Trophy } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Logo } from "@/components/Logo";
import { loadLobby, type LobbyRelay } from "@/lib/api";
import {
  CHALLENGES,
  EXAMPLE_EVENT,
  dailySlot,
  eventPhase,
  eventStatusLabel,
  formatClock,
  heistCountdown,
  heistRows,
  mockQuotes,
  exampleHeadline,
  SEASON_END,
  SEASON_START,
  slotPhase,
  slotWindow,
  upcomingSeason,
  weeklySlot,
  type HeistWindow,
  type Slot,
} from "@/lib/schedule";

type LobbyEvent = {
  id: string;
  partner: string;
  exampleOnly: boolean;
  room: string;
  prize: string;
  start: number;
  end: number;
};

type LobbyView = {
  source: "relay" | "fixture";
  note: string;
  now: number;
  live: boolean;
  secs: number;
  seasonStart: number;
  seasonEnd: number;
  heist: Record<HeistWindow, { name: string; points: number }[]>;
  quotes: { symbol: string; kind: string; mark: number; fundingBps: number; pnlMicros: number }[];
  challenges: { id: string; category: string; title: string; points: number }[];
  events: LobbyEvent[];
};

const ROOM_LABEL: Record<string, string> = {
  "vault-quarter": "Vault",
  "cup-stadium": "Stadium",
  "trading-market": "Trading floor",
  "war-room": "War room",
  "skin-forge": "Lobby",
};

const DOORS = [
  ["/games/heist-night", "Vault", DoorOpen],
  ["/village", "Trading floor", LineChart],
  ["/war-room", "War room", Shield],
  ["/games/trading-cup", "Stadium", Trophy],
] as const;

function fromRelay(relay: LobbyRelay, now: number): LobbyView {
  const clock = heistCountdown(now);
  return {
    source: "relay",
    note: "Relay. Testnet and mock mode.",
    now,
    live: clock.live,
    secs: clock.secs,
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
  const clock = heistCountdown(now);
  return {
    source: "fixture",
    note: reason === "unset" ? "Fixture. This build has no API URL." : "Fixture. The API did not answer.",
    now,
    live: clock.live,
    secs: clock.secs,
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
        start: EXAMPLE_EVENT.start,
        end: EXAMPLE_EVENT.end,
      },
    ],
  };
}

function retick(view: LobbyView, now: number): LobbyView {
  if (view.source === "fixture") return fromFixture(now, view.note.includes("no API") ? "unset" : "unreachable");
  const clock = heistCountdown(now);
  return { ...view, now, live: clock.live, secs: clock.secs };
}

function money(micros: number) {
  return (micros / 1_000_000).toFixed(2);
}

function SlotRow({ now, slot, title }: { now: number; slot: Slot; title: string }) {
  const phase = slotPhase(now, slot);
  const label = phase === "live" ? "Live" : phase === "ended" ? "Ended" : "Next";
  const when = slotWindow(slot.start, slot.end);
  return (
    <li className="lobby-slot">
      <span>{title}</span>
      <span className="lobby-when num">
        <span>{when.utc}</span>
        <span>{when.wat}</span>
      </span>
      <span className={phase === "live" ? "lobby-live" : "lobby-quiet"}>{label}</span>
    </li>
  );
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
        <header className="lobby-panel lobby-hero lobby-span-12">
          <h1>
            <Logo variant="hero" />
          </h1>
          <div>
            <p className="lede">The hall you arrive in. Nothing here is a move.</p>
            <p className="meta">Boards follow the UTC clock.</p>
          </div>
        </header>
        <section className="lobby-panel lobby-span-7" aria-busy="true">
          <p className="label">Schedule</p>
          <p className="lobby-count num">--:--:--</p>
          <p className="meta">Daily, weekly, and seasonal windows are UTC.</p>
        </section>
        <section className="lobby-panel lobby-span-5">
          <h2>Events</h2>
          <p className="meta">Events follow the UTC clock.</p>
        </section>
        <section className="lobby-panel lobby-span-7">
          <h2>Leaderboard</h2>
          <p className="meta">Scores follow the UTC clock.</p>
        </section>
        <section className="lobby-panel lobby-span-5">
          <h2>Venues</h2>
          <p className="meta">Mock tape. No order is sent. No key is used.</p>
        </section>
        <section className="lobby-panel lobby-span-7">
          <h2>War Room</h2>
          <p className="meta">Challenges follow the UTC clock.</p>
        </section>
        <section className="lobby-panel lobby-span-5">
          <h2>Doors</h2>
          <div className="lobby-doors">
            {DOORS.map(([href, label, Icon]) => (
              <a className="btn btn-secondary" key={href} href={href}>
                <Icon aria-hidden="true" strokeWidth={1.75} />
                {label}
              </a>
            ))}
          </div>
        </section>
      </>
    );
  }

  const rows = view.heist[windowName];
  const slots: Array<[string, Slot]> = [
    ["Daily", dailySlot(view.now)],
    ["Weekly", weeklySlot(view.now)],
    ["Seasonal", upcomingSeason(view.now, view.seasonStart, view.seasonEnd)],
  ];

  return (
    <>
      <header className="lobby-panel lobby-hero lobby-span-12 ui-in">
        <h1>
          <Logo variant="hero" />
        </h1>
        <div>
          <p className="lede">The hall you arrive in. Nothing here is a move. Boards, doors, and the clock.</p>
          <p className="meta">{view.note}</p>
        </div>
      </header>
      <section className="lobby-panel lobby-span-7 ui-in">
        <p className="label">Schedule</p>
        <p className="lobby-kicker">{view.live ? "Live" : "Next heist"}</p>
        <p className="lobby-count num" key={view.secs}>
          {formatClock(view.secs)}
        </p>
        <ul className="lobby-slots">
          {slots.map(([title, slot]) => (
            <SlotRow key={title} now={view.now} slot={slot} title={title} />
          ))}
        </ul>
        <p className="meta lobby-when">
          <span>{slotWindow(view.seasonStart, view.seasonEnd).utc}</span>
          <span>{slotWindow(view.seasonStart, view.seasonEnd).wat}</span>
        </p>
      </section>
      <section className="lobby-panel lobby-events ui-in">
        <h2>Events</h2>
        <ul className="lobby-stack">
          {view.events.map((event) => {
            const phase = eventPhase(view.now, event.start, event.end, event.exampleOnly);
            const when = slotWindow(event.start, event.end);
            const headline = event.exampleOnly ? exampleHeadline(view.now, event.start, event.end) : eventStatusLabel(phase);
            return (
              <li key={event.id}>
                <p className={phase === "live" ? "lobby-live" : "lobby-quiet"}>{headline}</p>
                <p>
                  {event.partner} · {ROOM_LABEL[event.room] ?? event.room}
                </p>
                <p className="meta">{event.prize}</p>
                <p className="lobby-when num">
                  <span>{when.utc}</span>
                  <span>{when.wat}</span>
                </p>
              </li>
            );
          })}
        </ul>
      </section>
      <section className="lobby-panel lobby-span-7 ui-in">
        <h2>Leaderboard</h2>
        <p className="meta">Mock scores from the API clock.</p>
        <div className="lobby-tabs" role="tablist" aria-label="Heist window">
          {(["today", "week", "season"] as const).map((id) => (
            <button key={id} type="button" role="tab" aria-selected={windowName === id} onClick={() => setWindowName(id)}>
              {id === "today" ? <Clock size={16} aria-hidden="true" /> : id === "week" ? <Calendar size={16} aria-hidden="true" /> : <Trophy size={16} aria-hidden="true" />}
              {id}
            </button>
          ))}
        </div>
        <ol className="lobby-rank ui-in" key={windowName}>
          {rows.length === 0 ? (
            <li>No scores in this window.</li>
          ) : (
            rows.map((row, index) => (
              <li key={row.name}>
                <span className="num">{index + 1}</span>
                <span>{row.name}</span>
                <span className="num">{row.points} points</span>
              </li>
            ))
          )}
        </ol>
        <p>
          <a href={`/leaderboard?window=${windowName}&room=vault`}>Full leaderboard</a>
        </p>
      </section>
      <section className="lobby-panel lobby-span-5 ui-in">
        <h2>Venues</h2>
        <ul className="lobby-stack">
          {view.quotes.map((row) => (
            <li className="lobby-quote" key={row.symbol}>
              <span>
                <span className="lobby-quiet">{row.kind}</span> {row.symbol}
              </span>
              <span className="num">{money(row.mark)}</span>
              <span className="meta">
                funding {row.fundingBps} bps · pnl {money(row.pnlMicros)}
              </span>
            </li>
          ))}
        </ul>
        <p className="meta">Mock tape. No order is sent. No key is used.</p>
      </section>
      <section className="lobby-panel lobby-span-7 ui-in">
        <h2>War Room</h2>
        <ul className="lobby-rank">
          {view.challenges.map((row) => (
            <li key={row.id}>
              <span className="lobby-quiet">{row.category}</span>
              <span>{row.title}</span>
              <span className="num">{row.points}</span>
            </li>
          ))}
        </ul>
      </section>
      <section className="lobby-panel lobby-span-5 ui-in">
        <h2>Doors</h2>
        <div className="lobby-doors">
          {DOORS.map(([href, label, Icon]) => (
            <a className="btn btn-secondary" key={href} href={href}>
              <Icon aria-hidden="true" strokeWidth={1.75} />
              {label}
            </a>
          ))}
        </div>
      </section>
    </>
  );
}
