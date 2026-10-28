"use client";

import { Calendar, Clock, Trophy } from "lucide-react";
import { useEffect, useState } from "react";
import {
  CHALLENGES,
  EXAMPLE_EVENT,
  eventPhase,
  eventStatusLabel,
  exampleEventLive,
  formatCountdown,
  heistCountdown,
  heistRows,
  localRange,
  mockQuotes,
  nextHeist,
  type HeistWindow,
} from "../lib/schedule";

function useNow() {
  const [now, setNow] = useState<number | null>(null);
  useEffect(() => {
    const tick = () => setNow(Math.floor(Date.now() / 1000));
    tick();
    const id = window.setInterval(tick, 1000);
    return () => window.clearInterval(id);
  }, []);
  return now;
}

export function CountdownChip({ compact = false }: { compact?: boolean }) {
  const now = useNow();
  if (now == null) {
    return (
      <span className="chip">
        <Clock size={14} aria-hidden="true" />
        Heist clock
      </span>
    );
  }
  const slot = nextHeist(now);
  const clock = heistCountdown(now);
  const label = clock.live ? `LIVE ${formatCountdown(clock.secs)}` : `Next heist in ${formatCountdown(clock.secs)}`;
  return (
    <span className="chip">
      <Clock size={14} aria-hidden="true" />
      {compact ? label : `${label} · ${slot.kind} · ${localRange(slot.start, slot.end)}`}
    </span>
  );
}

export function EventBanner({ room, compact = false }: { room?: string; compact?: boolean }) {
  const now = useNow();
  if (now == null || !exampleEventLive(now)) return null;
  if (room && room !== EXAMPLE_EVENT.room && room !== "map") return null;
  const label = eventStatusLabel(eventPhase(now, EXAMPLE_EVENT.start, EXAMPLE_EVENT.end, true));
  if (compact) {
    return (
      <span className="chip">
        <Trophy size={14} aria-hidden="true" />
        {label}
      </span>
    );
  }
  return (
    <p className="banner">
      {label} · {EXAMPLE_EVENT.partner} · {EXAMPLE_EVENT.roomLabel} · {EXAMPLE_EVENT.prize}
    </p>
  );
}

export function ScheduleList() {
  const now = useNow();
  if (now == null) {
    return (
      <ul className="schedule-list">
        <li>Season window is UTC.</li>
        <li>Daily 18:00 UTC, 60 minutes. Weekly Monday 18:00 UTC, 120 minutes.</li>
        <li>The next slot follows the UTC clock.</li>
      </ul>
    );
  }
  const slot = nextHeist(now);
  return (
    <ul className="schedule-list">
      <li>Season opens {localRange(1_790_812_800, 1_798_761_600)}</li>
      <li>Daily 18:00 UTC, 60 minutes. Weekly Monday 18:00 UTC, 120 minutes.</li>
      <li>
        Next {slot.kind} slot {localRange(slot.start, slot.end)}
      </li>
    </ul>
  );
}

export function HeistBoard() {
  const now = useNow();
  const [window, setWindow] = useState<HeistWindow>("today");
  const rows = now == null ? [] : heistRows(window, now);
  return (
    <section className="board">
      <div className="tabs">
        {(["today", "week", "season"] as const).map((id) => (
          <button key={id} type="button" onClick={() => setWindow(id)} aria-pressed={window === id}>
            {id === "today" ? <Clock size={16} aria-hidden="true" /> : id === "week" ? <Calendar size={16} aria-hidden="true" /> : <Trophy size={16} aria-hidden="true" />}
            {id}
          </button>
        ))}
      </div>
      <ol>
        {now == null ? (
          <li>Scores follow the UTC clock.</li>
        ) : rows.length === 0 ? (
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
  );
}

export function QuoteBoard() {
  const now = useNow();
  if (now == null) {
    return (
      <section className="board">
        <p className="muted">Mock tape. No order is sent. No key is used.</p>
      </section>
    );
  }
  const rows = mockQuotes(now);
  return (
    <section className="board">
      <ul>
        {rows.map((row) => (
          <li key={row.symbol}>
            {row.kind} {row.symbol} {(row.mark / 1_000_000).toFixed(2)} · funding {row.fundingBps} bps · pnl {(row.pnlMicros / 1_000_000).toFixed(2)}
          </li>
        ))}
      </ul>
      <p className="muted">Mock tape. No order is sent. No key is used.</p>
    </section>
  );
}

export function ChallengeBoard() {
  return (
    <section className="board">
      <ul>
        {CHALLENGES.map((row) => (
          <li key={row.id}>
            {row.category} · {row.title} · {row.points}
          </li>
        ))}
      </ul>
    </section>
  );
}
