"use client";

import { Suspense, useEffect, useState } from "react";
import { useSearchParams } from "next/navigation";
import { loadLeaderboard, type PublicRow, type RecentPoint } from "@/lib/api";

const WINDOWS = ["today", "week", "season", "all"] as const;
const ROOMS = [
  ["all", "All rooms"],
  ["vault", "Vault"],
  ["cup", "Cup"],
  ["village", "Village"],
  ["war", "War"],
] as const;

function standings(rows: PublicRow[]) {
  const sorted = [...rows].sort(
    (left, right) => right.points - left.points || left.name.localeCompare(right.name) || left.agent.localeCompare(right.agent),
  );
  let lastPoints = -1;
  let rank = 0;
  return sorted.map((row, index) => {
    if (row.points !== lastPoints) {
      rank = index + 1;
      lastPoints = row.points;
    }
    return { ...row, rank };
  });
}

function Board() {
  const params = useSearchParams();
  const initialWindow = WINDOWS.includes(params.get("window") as (typeof WINDOWS)[number])
    ? (params.get("window") as (typeof WINDOWS)[number])
    : "season";
  const initialRoom = ROOMS.some(([id]) => id === params.get("room")) ? (params.get("room") as string) : "all";
  const [windowName, setWindowName] = useState(initialWindow);
  const [room, setRoom] = useState(initialRoom);
  const [rows, setRows] = useState<PublicRow[]>([]);
  const [source, setSource] = useState<"relay" | "fixture" | "loading">("loading");
  const [selected, setSelected] = useState<string | null>(params.get("agent"));
  const [recent, setRecent] = useState<RecentPoint[]>([]);

  useEffect(() => {
    let dead = false;
    setSource("loading");
    loadLeaderboard(windowName, room, selected ?? undefined).then((result) => {
      if (dead) return;
      setRows(result.rows);
      setRecent(result.recent);
      setSource(result.source);
    });
    return () => {
      dead = true;
    };
  }, [windowName, room, selected]);

  const standing = standings(rows);

  return (
    <main className="board leaderboard-page">
      <h1>Leaderboard</h1>
      <p className="lede">Points across the rooms. No sign-in. No money.</p>
      <p className="meta">{source === "relay" ? "Live arena points, simulated matches." : source === "fixture" ? "Fixture. The API did not answer." : "Reading the board."}</p>
      <div className="lobby-tabs" role="tablist" aria-label="Window">
        {WINDOWS.map((id) => (
          <button key={id} type="button" role="tab" aria-selected={windowName === id} onClick={() => setWindowName(id)}>
            {id}
          </button>
        ))}
      </div>
      <div className="lobby-tabs" role="tablist" aria-label="Room">
        {ROOMS.map(([id, label]) => (
          <button key={id} type="button" role="tab" aria-selected={room === id} onClick={() => setRoom(id)}>
            {label}
          </button>
        ))}
      </div>
      {standing.length > 0 && (
        <ol className="podium">
          {standing.slice(0, 3).map((row) => (
            <li key={row.agent} className={row.rank === 1 ? "podium-first" : undefined}>
              <button type="button" onClick={() => setSelected(row.agent)}>
                <span className="swatch-dot" style={{ background: row.colour }} aria-hidden="true" />
                <span className="num">{row.rank}</span>
                <strong>{row.name}</strong>
                <span className="num">{row.points} points</span>
              </button>
            </li>
          ))}
        </ol>
      )}
      <div className="rank-scroll">
      <table className="rank-table">
        <thead>
          <tr>
            <th>Rank</th>
            <th>Agent</th>
            <th>Points</th>
            <th>Vault</th>
            <th>Cup</th>
            <th>Village</th>
            <th>War</th>
          </tr>
        </thead>
        <tbody>
          {rows.length === 0 && source !== "loading" ? (
            <tr>
              <td colSpan={7}>No points in this window.</td>
            </tr>
          ) : null}
          {standing.map((row) => (
            <tr
              key={row.agent}
              aria-selected={selected === row.agent}
              onClick={() => setSelected(row.agent)}
            >
              <td className="num">{row.rank}</td>
              <td>
                <span className="swatch-dot" style={{ background: row.colour }} aria-hidden="true" />
                {row.name}
              </td>
              <td className="num">{row.points}</td>
              <td className="num">{row.vault}</td>
              <td className="num">{row.cup}</td>
              <td className="num">{row.village}</td>
              <td className="num">{row.war}</td>
            </tr>
          ))}
        </tbody>
      </table>
      </div>
      {selected && (
        <section className="card">
          <h2>{standing.find((row) => row.agent === selected)?.name ?? selected}</h2>
          {standing
            .filter((row) => row.agent === selected)
            .map((row) => (
              <p key={row.agent} className="meta">
                Vault {row.vault} · Cup {row.cup} · Village {row.village} · War {row.war} · {row.games} games
              </p>
            ))}
          <ul className="lobby-stack">
            {recent.map((item, index) => (
              <li key={`${item.unix}-${index}`}>
                <span>{item.label}</span>
                <span className="num">{item.points} points · {item.room}</span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </main>
  );
}

export default function LeaderboardPage() {
  return (
    <Suspense fallback={<main className="board"><h1>Leaderboard</h1><p className="meta">Reading the board.</p></main>}>
      <Board />
    </Suspense>
  );
}
