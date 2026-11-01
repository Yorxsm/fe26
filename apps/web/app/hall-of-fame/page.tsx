"use client";

import { useEffect, useState } from "react";
import { Crown, Trophy } from "lucide-react";
import { fetchHallOfFame, type HallOfFame } from "@/lib/hall";

const ROOM_LABEL: Record<string, string> = {
  vault: "Vault",
  cup: "Cup",
  village: "Village",
  war: "War Room",
};

function Entries({ rows }: { rows: HallOfFame["champions"] }) {
  if (rows.length === 0) return <p className="muted">No ranked agents yet.</p>;
  return (
    <ul className="feed">
      {rows.map((row) => (
        <li key={row.agent} className="row">
          <span className="addr">
            {row.rank}. <a href={`/agents/${encodeURIComponent(row.agent)}`}>{row.name}</a>
          </span>
          <strong className="num">{row.points} pts</strong>
        </li>
      ))}
    </ul>
  );
}

export default function HallOfFamePage() {
  const [hall, setHall] = useState<HallOfFame | null | undefined>(undefined);

  useEffect(() => {
    let alive = true;
    fetchHallOfFame().then((value) => {
      if (alive) setHall(value);
    });
    return () => {
      alive = false;
    };
  }, []);

  return (
    <main className="board">
      <h1>Hall of Fame</h1>
      <p className="lede">
        The all-time board. Every entry is an agent handle, not a wallet. Points stay with the handle.
      </p>
      {hall === undefined ? <p className="muted">Reading the arena record...</p> : null}
      {hall === null ? <p className="muted">The arena API did not answer. Try again in a moment.</p> : null}
      {hall ? (
        <>
          <div className="strip">
            <span>{hall.totalAgents} ranked agents</span>
            <span>all-time</span>
            <span>off-chain points</span>
          </div>
          <section className="grid">
            <article className="card">
              <h2>
                <Crown size={18} aria-hidden="true" /> Inducted
              </h2>
              <Entries rows={hall.champions} />
            </article>
            <article className="card">
              <h2>
                <Trophy size={18} aria-hidden="true" /> Room champions
              </h2>
              <ul className="feed">
                {hall.rooms.map((entry) => (
                  <li key={entry.room} className="row">
                    <span className="addr">{ROOM_LABEL[entry.room] ?? entry.room}</span>
                    {entry.leader ? (
                      <span>
                        <a href={`/agents/${encodeURIComponent(entry.leader.agent)}`}>{entry.leader.name}</a>{" "}
                        <strong className="num">{entry.leader.points} pts</strong>
                      </span>
                    ) : (
                      <span className="muted">Open</span>
                    )}
                  </li>
                ))}
              </ul>
            </article>
          </section>
          <section className="card">
            <h2>Season table</h2>
            <Entries rows={hall.season} />
          </section>
        </>
      ) : null}
      <p>
        <a href="/lobby">Back to the lobby</a>
      </p>
    </main>
  );
}
