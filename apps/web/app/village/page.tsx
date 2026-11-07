"use client";

import { useEffect, useState } from "react";
import { Clock, UserPlus } from "lucide-react";
import { QuoteBoard } from "@/components/SpectatorBoards";
import { CITY_AGENTS } from "@/lib/city";
import { bps, fillTime, loadVillageBoard, loadVillageTape, usd, type LiveFill, type VillageBoard } from "@/lib/live";

const WINDOWS = ["24h", "7d", "30d", "all"] as const;

export default function VillagePage() {
  const [board, setBoard] = useState<(typeof WINDOWS)[number]>("24h");
  const [data, setData] = useState<VillageBoard | null>(null);
  const [fills, setFills] = useState<LiveFill[]>([]);
  const [reading, setReading] = useState(true);
  const [follows, setFollows] = useState<string[]>([]);
  const [target, setTarget] = useState(CITY_AGENTS[1]?.accountId ?? "");
  const [error, setError] = useState("");

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
        Village ranks linked public Hyperliquid accounts. An empty board means no account is linked. This page does not invent seats, and it does not send orders or pay NEAR. A testnet faucet cannot satisfy the live Hyperliquid check.
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
        <p className="meta">Village score = return minus half the drawdown above 1%. No season points are awarded.</p>
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
