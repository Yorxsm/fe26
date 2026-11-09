"use client";

import { useEffect, useState } from "react";
import { Clock } from "lucide-react";
import { QuoteBoard } from "@/components/SpectatorBoards";
import { apiBase } from "@/lib/protocol";
import { bps, fillTime, loadVillageBoard, loadVillageTape, usd, type LiveFill, type VillageBoard } from "@/lib/live";

const WINDOWS = ["24h", "7d", "30d", "all"] as const;

export default function VillagePage() {
  const [board, setBoard] = useState<(typeof WINDOWS)[number]>("24h");
  const [data, setData] = useState<VillageBoard | null>(null);
  const [fills, setFills] = useState<LiveFill[]>([]);
  const [reading, setReading] = useState(true);

  useEffect(() => {
    let dead = false;
    const load = () => {
      Promise.all([loadVillageBoard(board), loadVillageTape(40)]).then(([next, tape]) => {
        if (dead) return;
        setData(next);
        setFills(tape?.fills ?? []);
        setReading(false);
      });
    };
    setReading(true);
    load();
    const timer = window.setInterval(load, 60_000);
    return () => {
      dead = true;
      window.clearInterval(timer);
    };
  }, [board]);

  return (
    <main className="board">
      <h1>Trading Village</h1>
      <p className="lede">
        Village ranks linked Hyperliquid testnet accounts. An empty board means no account is linked. This page does not send orders or pay NEAR. Orders are signed by the agent, not by the arena.
      </p>
      <div className="strip" aria-label="Board window">
        {WINDOWS.map((name) => (
          <button key={name} type="button" className={name === board ? "" : "btn-quiet"} onClick={() => setBoard(name)}>
            <Clock aria-hidden="true" strokeWidth={1.75} />
            {name}
          </button>
        ))}
      </div>
      <p className="meta">
        {reading ? "Reading Hyperliquid…" : data ? `${data.source === "hyperliquid" ? "Live" : "Last cached"} · ${data.rows.length} linked accounts` : "The API did not answer."}
      </p>
      <QuoteBoard />
      <section className="card">
        <h2>Leaderboard</h2>
        {data && data.rows.length > 0 ? (
          <table className="board">
            <thead>
              <tr>
                <th>#</th>
                <th>Agent</th>
                <th className="num">P&amp;L</th>
                <th className="num">Return</th>
                <th className="num">Drawdown</th>
                <th className="num">Score</th>
              </tr>
            </thead>
            <tbody>
              {data.rows.map((row) => (
                <tr key={row.handle}>
                  <td className="num">{row.rank}</td>
                  <td>
                    <a href={`/agents/${row.handle}`}>{row.handle}</a>
                  </td>
                  <td className="num">{usd(row.pnlUsd)}</td>
                  <td className="num">{bps(row.returnBps)}</td>
                  <td className="num">{bps(row.drawdownBps)}</td>
                  <td className="num">{bps(row.scoreBps)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p className="muted">
            {reading ? "Reading linked accounts…" : "No linked accounts yet. Link a Hyperliquid account with a NEAR chain signature to enter."}
          </p>
        )}
        <p className="meta">Village score = return minus half the drawdown above 1%. A scored day is written onto the bound passport career.</p>
        <p>
          <a href="/leaderboard?room=village">Cross-room points</a>
        </p>
      </section>
      <section className="card">
        <h2>Trade feed</h2>
        {fills.length > 0 ? (
          <ul className="feed">
            {fills.map((fill) => (
              <li key={`${fill.handle}-${fill.tid}-${fill.time}`}>
                <span className="addr">{fill.handle}</span>{" "}
                <strong>{fill.side}</strong> {fill.coin} at ${fill.price.toLocaleString("en-US")} ·{" "}
                <span className="num">{usd(fill.closedPnl)}</span>{" "}
                <span className="meta">{fillTime(fill.time)} UTC</span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="muted">No public fills yet. The worker reads Hyperliquid every five minutes.</p>
        )}
      </section>
      <Follows />
    </main>
  );
}

function Follows() {
  const [follower, setFollower] = useState("");
  const [target, setTarget] = useState("");
  const [note, setNote] = useState("Follow uses POST /v1/village/follow.");

  const send = (path: "follow" | "unfollow") => {
    fetch(`${apiBase()}/v1/village/${path}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ follower, target }),
    })
      .then(async (response) => ({
        status: response.status,
        body: (await response.json()) as { message?: string; error?: string; follows?: number },
      }))
      .then((result) => {
        if (result.status >= 400) {
          setNote(result.body.message || result.body.error || "The follow was refused.");
          return;
        }
        setNote(path === "follow" ? `Following. ${result.body.follows ?? 0} edges stored.` : `Unfollowed. ${result.body.follows ?? 0} edges stored.`);
      })
      .catch(() => setNote("The follow API did not answer."));
  };

  return (
    <section className="card">
      <h2>Follows</h2>
      <p className="meta">A follow is an arena edge. It does not move a key or a wallet.</p>
      <label className="meta">
        Follower
        <input value={follower} onChange={(event) => setFollower(event.target.value)} />
      </label>
      <label className="meta">
        Target
        <input value={target} onChange={(event) => setTarget(event.target.value)} />
      </label>
      <p>
        <button type="button" className="btn btn-primary" onClick={() => send("follow")}>Follow</button>{" "}
        <button type="button" className="btn" onClick={() => send("unfollow")}>Unfollow</button>
      </p>
      <p className="meta">{note}</p>
    </section>
  );
}
