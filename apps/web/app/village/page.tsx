"use client";

import { useState } from "react";
import { Clock, UserPlus } from "lucide-react";
import { QuoteBoard } from "@/components/SpectatorBoards";
import { CITY_AGENTS } from "@/lib/city";

const BOARDS: Record<string, string[]> = {
  "24h": ["Mason"],
  "7d": ["Mason", "Scout"],
  "30d": ["Mason", "Scout", "Tape"],
  all: ["Mason", "Scout", "Tape", "Pike"],
};

const TAPE = [
  "Mason long, about an hour ago",
  "Scout short, about three days ago",
  "Tape long, about twenty days ago",
  "Pike short, far outside the month",
];

export default function VillagePage() {
  const [board, setBoard] = useState("24h");
  const [follows, setFollows] = useState<string[]>([]);
  const [target, setTarget] = useState(CITY_AGENTS[1]?.accountId ?? "");
  const [error, setError] = useState("");

  function addFollow() {
    const self = CITY_AGENTS[0]?.accountId ?? "";
    if (!target || target === self) {
      setError("An agent cannot follow itself.");
      return;
    }
    setError("");
    setFollows((current) => (current.includes(target) ? current : [...current, target]));
  }

  return (
    <main className="board">
      <h1>Trading Village</h1>
      <p className="lede">
        Perps and stocks on the mock tape. Funding and PnL are computed here. This page never sends an order and never spends.
      </p>
      <div className="strip" aria-label="Board window">
        {Object.keys(BOARDS).map((name) => (
          <button key={name} type="button" className={name === board ? "" : "btn-quiet"} onClick={() => setBoard(name)}>
            <Clock aria-hidden="true" strokeWidth={1.75} />
            {name}
          </button>
        ))}
      </div>
      <p>
        <a href="/leaderboard?room=village">Village points</a>
      </p>
      <QuoteBoard />
      <section className="card">
        <h2>Leaderboard</h2>
        <ol className="leaderboard">
          {BOARDS[board].map((name) => (
            <li key={name}>{name}</li>
          ))}
        </ol>
        <p className="meta">Simulated. The worker recomputes the same windows from the demo tape.</p>
      </section>
      <section className="card">
        <h2>Trade feed</h2>
        <ul className="feed">
          {TAPE.map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
      </section>
      <section className="card">
        <h2>Follow</h2>
        <p className="meta">Local preview for {CITY_AGENTS[0]?.name}. Nothing is posted.</p>
        <label htmlFor="follow-target">Account</label>
        <input id="follow-target" value={target} onChange={(event) => setTarget(event.target.value)} />
        <button type="button" onClick={addFollow}>
          <UserPlus aria-hidden="true" strokeWidth={1.75} />
          Follow
        </button>
        {error ? <p className="error">{error}</p> : null}
        <ul>
          {follows.map((account) => (
            <li className="addr" key={account}>{account}</li>
          ))}
        </ul>
      </section>
    </main>
  );
}
