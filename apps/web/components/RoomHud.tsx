"use client";

import { useEffect, useState } from "react";
import { ArrowLeft, Info, Trophy } from "lucide-react";
import { useRoomClocks } from "@/lib/clock";
import { apiBase } from "@/lib/protocol";
import { formatCountdown } from "@/lib/schedule";

function plainPoints(room: string, book: Record<string, unknown>) {
  const asRecord = (value: unknown) => (value && typeof value === "object" ? (value as Record<string, unknown>) : null);
  if (room === "vault") {
    const row = asRecord(book.vault);
    if (!row) return null;
    return "Both seats submit a secret order: scout, cover, crack, steal, or sabotage. Crack opens a lock, steal takes loot, cover lowers heat, and sabotage cancels a crack or steal.";
  }
  if (room === "cup") {
    const row = asRecord(book.cup);
    const places = Array.isArray(row?.place) ? row.place.filter((value) => typeof value === "number") : [];
    if (!row) return null;
    return places.length
      ? `Top five score ${places.join(", ")}. Ties share the place; only positive risk-adjusted returns score.`
      : "Top five share the Cup points.";
  }
  if (room === "village") {
    const row = asRecord(book.village);
    if (!row) return null;
    return "The Village ranks risk-adjusted P&L. It does not award season points.";
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
    works: "Two roles, hidden objectives, and simultaneous secret orders. Best of 5 rounds.",
    rules: "vault",
  },
  "cup-stadium": {
    slug: "cup",
    title: "Stadium",
    works: "The Cup is one weekly live Hyperliquid round. Eligible seats score by risk-adjusted return.",
    rules: "cup",
  },
  "trading-village": {
    slug: "village",
    title: "Trading floor",
    works: "The floor is always open. Village ranks public Hyperliquid P&L; it does not award points.",
    rules: "village",
  },
  "war-room": {
    slug: "war",
    title: "War room",
    works: "Solve a sandbox, then submit the flag it gives that agent. Points are 1, 2, or 3.",
    rules: "war",
  },
  "skin-forge": {
    slug: "all",
    title: "Lobby",
    works: "The hall. Boards for the other rooms. Nothing here awards points.",
    rules: "",
  },
  "oracle-plaza": {
    slug: "all",
    title: "Season hall",
    works: "The season board and the matches the arena has played. This room does not award points.",
    rules: "",
  },
};

export type RoomMatch = {
  matchId: string;
  arenaId: string;
  status: string;
  turn: number;
  deadlineAt: string | null;
  seats: string[];
  last: { type: string; handle: string; points: number } | null;
  vault: { round: number; rounds: number; ended: boolean; scores: Array<{ handle: string; roundWins: number }> } | null;
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

function localWhen(iso: string) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleString(undefined, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });
}

function liveDeadline(matches: RoomMatch[] | null, slug: string) {
  if (!matches) return "";
  const mine = slug === "all" ? matches : matches.filter((match) => match.arenaId === slug);
  const live = mine.find((match) => match.status === "live" && match.deadlineAt);
  return live?.deadlineAt ? ` Deadline ${localWhen(live.deadlineAt)}.` : "";
}

export type WarClaimRow = { handle: string; points: number; challenge: string };

function displayName(handle: string, names: Record<string, string>) {
  const known = names[handle];
  if (known) return known;
  return handle
    .split("-")
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function peopleHere(matches: RoomMatch[] | null, queues: Record<string, string[]>, slug: string, names: Record<string, string>) {
  if (!matches || slug === "all") return [];
  const seated = matches.filter((match) => match.status === "live" && match.arenaId === slug).flatMap((match) => match.seats);
  const waiting = queues[slug] ?? [];
  return [...new Set([...seated, ...waiting].filter(Boolean).map((handle) => displayName(handle, names)))];
}

function roomActivity(matches: RoomMatch[] | null, claims: WarClaimRow[], slug: string) {
  if (matches === null) return { summary: "Reading the arena…", rows: [] as string[] };
  const mine = slug === "all" ? matches : matches.filter((match) => match.arenaId === slug);
  const live = mine.find((match) => match.status === "live");
  const done = mine.find((match) => match.status !== "live");
  const focus = live ?? done;
  if (!focus) {
    if (slug === "war" && claims.length) {
      return {
        summary: "Recent solves.",
        rows: claims.slice(0, 5).map((claim) => `${claim.handle} ${claim.challenge} · ${claim.points}`),
      };
    }
    return { summary: "No one yet", rows: [] as string[] };
  }
  const seats = focus.seats.filter(Boolean).join(" · ") || "Open seats";
  const vault = focus.vault;
  const summary = live
    ? vault
      ? `Round ${vault.round} of ${vault.rounds}. ${vault.scores.map((seat) => `${seat.handle} ${seat.roundWins}`).join(" · ") || seats}. Turn ${live.turn}.`
      : `${seats}. Turn ${live.turn}.`
    : `Last match · ${focus.status}. ${seats}.`;
  const rows = mine
    .map((match) => match.last)
    .filter((row): row is NonNullable<RoomMatch["last"]> => Boolean(row?.type && row.handle))
    .slice(0, 5)
    .map((row) => (row.points > 0 ? `${row.handle} ${row.type} · ${row.points}` : `${row.handle} ${row.type}`));
  return { summary, rows };
}

export function RoomHud({
  roomId,
  tip,
  matches,
  queues,
  claims,
  names,
  onLeave,
}: {
  roomId: string;
  tip: string | null;
  matches: RoomMatch[] | null;
  queues: Record<string, string[]>;
  claims: WarClaimRow[];
  names: Record<string, string>;
  onLeave: () => void;
}) {
  const about = ABOUT[roomId];
  const [open, setOpen] = useState(false);
  const [full, setFull] = useState(false);
  const [sheet, setSheet] = useState(false);
  useEffect(() => {
    const narrow = window.matchMedia("(max-width: 800px)");
    const apply = () => setSheet(narrow.matches);
    apply();
    narrow.addEventListener("change", apply);
    return () => narrow.removeEventListener("change", apply);
  }, []);
  const [sheetOpen, setSheetOpen] = useState(false);
  const [rules, setRules] = useState<string>("");
  const [rows, setRows] = useState<RoomRow[] | null>(null);
  const [boardError, setBoardError] = useState(false);
  const clocks = useRoomClocks();
  const slug = about?.slug ?? "all";
  const slot = clocks[slug];
  const activity = roomActivity(matches, claims, slug);
  const here = peopleHere(matches, queues, slug, names);

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
    <div className={sheet && !sheetOpen ? "room-hud is-sheet" : "room-hud"}>
      {sheet ? (
        <button type="button" className="btn btn-secondary room-sheet-toggle" onClick={() => setSheetOpen((value) => !value)}>
          {sheetOpen ? "Hide room panel" : "Room panel"}
        </button>
      ) : null}
      <button type="button" className="btn room-leave" onClick={onLeave}>
        <ArrowLeft aria-hidden="true" strokeWidth={1.75} />
        Leave room
      </button>
      <button type="button" className="btn btn-secondary" aria-expanded={open} onClick={() => setOpen((value) => !value)}>
        <Info aria-hidden="true" strokeWidth={1.75} />
        {about.title}
      </button>
      <p className="room-goal">{about.works}</p>
      <section className={full ? "room-board is-full" : "room-board"} aria-label={`${about.title} leaderboard`}>
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
        <button type="button" className="btn btn-secondary room-link" onClick={() => setFull((value) => !value)}>
          <Trophy aria-hidden="true" strokeWidth={1.75} />
          {full ? "Close board" : "Full board"}
        </button>
      </section>
      <section className="room-activity" aria-label={`${about.title} activity`}>
        <h2>What's happening</h2>
        <p>{activity.summary}{liveDeadline(matches, slug)}</p>
        {activity.rows.length ? (
          <ul>
            {activity.rows.map((row) => (
              <li key={row}>{row}</li>
            ))}
          </ul>
        ) : activity.summary === "No one yet" ? null : (
          <p className="meta">No moves in this room yet.</p>
        )}
        <p>Inside now: {here.length ? here.join(", ") : "No one yet"}</p>
      </section>
      {open && (
        <section className="room-panel">
          <h2>{about.title}</h2>
          <p>{about.works}</p>
          {roomId !== "skin-forge" && slot ? (
            <p className="meta">{`${slot.live ? "Live" : "Next"} · ${formatCountdown(slot.secs)}`}</p>
          ) : null}
          {rules ? <p className="meta">{rules}</p> : null}
          {tip ? <p className="meta">{tip}</p> : null}
        </section>
      )}
    </div>
  );
}
