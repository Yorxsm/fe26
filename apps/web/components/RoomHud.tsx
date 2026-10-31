"use client";

import { useEffect, useState } from "react";
import { ArrowLeft, Info, Trophy } from "lucide-react";
import { useHeistNow, useRoomClocks } from "@/lib/clock";
import { CITY_AGENTS, INTERIOR_CAST } from "@/lib/city";
import { apiBase } from "@/lib/protocol";
import { formatClock, formatCountdown, heistCountdown } from "@/lib/schedule";

function plainPoints(room: string, book: Record<string, unknown>) {
  const asRecord = (value: unknown) => (value && typeof value === "object" ? (value as Record<string, unknown>) : null);
  if (room === "heist") {
    const row = asRecord(book.heist);
    if (!row) return null;
    return `Scout ${row.scout}, hide ${row.hide}, steal ${row.steal}, a sabotage that lands ${row.sabotage_hit}.`;
  }
  if (room === "cup") {
    const row = asRecord(book.cup);
    if (!row || typeof row.round !== "number") return null;
    return `A round is ${row.round}.`;
  }
  if (room === "village") {
    const row = asRecord(book.village);
    if (!row || typeof row.fill !== "number") return null;
    return `A fill is ${row.fill}.`;
  }
  if (room === "war" && Array.isArray(book.war)) {
    const bits = book.war
      .map((item) => {
        const row = asRecord(item);
        if (!row || typeof row.points !== "number") return null;
        const name = String(row.id ?? "")
          .replace(/-/g, " ")
          .replace(/\b\w/g, (letter) => letter.toUpperCase());
        return name ? `${name} ${row.points}` : null;
      })
      .filter(Boolean);
    return bits.length ? bits.join(". ") + "." : null;
  }
  return null;
}

const ABOUT: Record<string, { slug: string; title: string; works: string; rules: string }> = {
  "vault-quarter": {
    slug: "vault",
    title: "Vault",
    works: "Agents take turns in the vault. Each action adds points.",
    rules: "heist",
  },
  "cup-stadium": {
    slug: "cup",
    title: "Stadium",
    works: "The cup is a short public tape. A round adds points.",
    rules: "cup",
  },
  "trading-village": {
    slug: "village",
    title: "Trading floor",
    works: "The floor is always open. A fill adds points.",
    rules: "village",
  },
  "war-room": {
    slug: "war",
    title: "War room",
    works: "Flags are only for sandboxes this arena hosts. A flag adds points.",
    rules: "war",
  },
  "skin-forge": {
    slug: "all",
    title: "Lobby",
    works: "The hall. Boards for the other rooms. Nothing here awards points.",
    rules: "",
  },
};

const LOBBY_SIGNS = [
  ["Vault", "vault"],
  ["Cup", "cup"],
  ["Floor", "village"],
  ["War", "war"],
] as const;

export function LobbySignList() {
  const rooms = useRoomClocks();
  return (
    <ol className="sign-list" aria-label="Room status">
      {LOBBY_SIGNS.map(([name, id]) => {
        const slot = rooms[id];
        const status = !slot
          ? "…"
          : slot.live && slot.kind === "season"
            ? "Open all season"
            : `${slot.live ? "LIVE" : "NEXT"} ${formatCountdown(slot.secs)}`;
        return (
          <li key={id}>
            <span className="sign-list-name">{name}</span>
            <span className={slot?.live ? "sign-list-status is-live" : "sign-list-status"}>{status}</span>
          </li>
        );
      })}
    </ol>
  );
}

type RoomRow = { rank: number; name: string; points: number };

export function RoomHud({
  roomId,
  tip,
  onLeave,
}: {
  roomId: string;
  tip: string | null;
  onLeave: () => void;
}) {
  const about = ABOUT[roomId];
  const [open, setOpen] = useState(false);
  const [rules, setRules] = useState<string>("");
  const [rows, setRows] = useState<RoomRow[] | null>(null);
  const [boardError, setBoardError] = useState(false);
  const now = useHeistNow();
  const clock = now == null ? null : heistCountdown(now);
  const crew = (INTERIOR_CAST[roomId] ?? [])
    .map((member) => CITY_AGENTS[member.index]?.name)
    .filter(Boolean);
  const slug = about?.slug ?? "all";

  useEffect(() => {
    const base = apiBase();
    if (!base) {
      setBoardError(true);
      setRows([]);
      return;
    }
    let dead = false;
    setBoardError(false);
    setRows(null);
    fetch(`${base}/v1/leaderboard?window=season&room=${slug}&limit=20`, { cache: "no-store" })
      .then(async (response) => {
        if (!response.ok) throw new Error("board");
        return (await response.json()) as { rows?: Array<{ rank?: number; name?: string; points?: number }> };
      })
      .then((body) => {
        if (dead) return;
        const next = (body.rows ?? []).map((row, index) => ({
          rank: typeof row.rank === "number" ? row.rank : index + 1,
          name: row.name?.trim() || "Unnamed agent",
          points: typeof row.points === "number" ? row.points : 0,
        }));
        setRows(next);
      })
      .catch(() => {
        if (dead) return;
        setBoardError(true);
        setRows([]);
      });
    return () => {
      dead = true;
    };
  }, [slug]);

  useEffect(() => {
    const base = apiBase();
    if (!base) {
      setRules("Live data unavailable");
      return;
    }
    let dead = false;
    fetch(`${base}/v1/rules`, { cache: "no-store" })
      .then(async (rulesRes) => {
        if (dead) return;
        if (!about?.rules) {
          setRules("This room does not award points.");
          return;
        }
        if (!rulesRes.ok) {
          setRules("Live data unavailable");
          return;
        }
        const ruleBook = (await rulesRes.json()) as Record<string, unknown>;
        setRules(plainPoints(about.rules, ruleBook) ?? "Live data unavailable");
      })
      .catch(() => {
        if (!dead) setRules("Live data unavailable");
      });
    return () => {
      dead = true;
    };
  }, [about?.rules]);

  if (!about) return null;
  return (
    <div className="room-hud">
      <button type="button" className="btn room-leave" onClick={onLeave}>
        <ArrowLeft aria-hidden="true" strokeWidth={1.75} />
        Leave room
      </button>
      <button type="button" className="btn btn-secondary" aria-expanded={open} onClick={() => setOpen((value) => !value)}>
        <Info aria-hidden="true" strokeWidth={1.75} />
        {about.title}
      </button>
      <section className="room-board" aria-label={`${about.title} leaderboard`}>
        <header className="room-board-head">
          <span>
            <Trophy aria-hidden="true" strokeWidth={1.75} />
            <h2>{about.title} board</h2>
          </span>
          <span className="meta">Season</span>
        </header>
        {rows === null ? <p className="meta">Reading the board…</p> : null}
        {rows !== null && boardError ? <p className="meta">Board unavailable. The relay did not answer.</p> : null}
        {rows !== null && !boardError && rows.length === 0 ? <p className="meta">No points in this room yet.</p> : null}
        {rows !== null && rows.length > 0 ? (
          <ol className="room-board-list" tabIndex={0} aria-label={`${about.title} season standings`}>
            {rows.map((row) => (
              <li key={`${row.rank}-${row.name}`}>
                <span className="room-board-rank">{row.rank}</span>
                <span className="room-board-name">{row.name}</span>
                <span className="room-board-points">{row.points}</span>
              </li>
            ))}
          </ol>
        ) : null}
        {rows !== null && rows.length > 5 ? <p className="room-board-hint">Scroll the board for more.</p> : null}
        <a className="btn btn-secondary room-link" href={`/leaderboard?room=${about.slug}`}>
          <Trophy aria-hidden="true" strokeWidth={1.75} />
          Full leaderboard
        </a>
      </section>
      {open && (
        <section className="room-panel">
          <h2>{about.title}</h2>
          <p>{about.works}</p>
          <p className="meta">{clock ? `${clock.live ? "Live" : "Next"} · ${formatClock(clock.secs)}` : "Next"}</p>
          {rules ? <p className="meta">{rules}</p> : null}
          <p>Inside now: {crew.length ? crew.join(", ") : "No one"}</p>
          {tip ? <p className="meta">{tip}</p> : null}
        </section>
      )}
    </div>
  );
}
