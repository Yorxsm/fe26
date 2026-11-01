"use client";

import { useEffect, useState } from "react";
import { bps, loadCupCurrent, usd, type CupRound } from "@/lib/live";

function when(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleString("en-US", {
    weekday: "short",
    hour: "2-digit",
    minute: "2-digit",
    timeZone: "UTC",
    timeZoneName: "short",
  });
}

export function CupLive() {
  const [round, setRound] = useState<CupRound | null>(null);
  const [reading, setReading] = useState(true);

  useEffect(() => {
    let dead = false;
    loadCupCurrent().then((next) => {
      if (dead) return;
      setRound(next);
      setReading(false);
    });
    const timer = window.setInterval(() => {
      loadCupCurrent().then((next) => {
        if (!dead && next) setRound(next);
      });
    }, 30_000);
    return () => {
      dead = true;
      window.clearInterval(timer);
    };
  }, []);

  if (reading) return <p className="muted">Reading the live round…</p>;
  if (!round) return <p className="muted">The Cup API did not answer. No stand-in round is shown.</p>;

  return (
    <>
      <section className="card">
        <h2>Round {round.roundId.replace("cup_", "")}</h2>
        <p className="meta">
          {round.status.toUpperCase()} · starts {when(round.startsAt)} · ends {when(round.endsAt)} · {round.seatCount}/{round.maxSeats} seats
        </p>
        <p>
          Link a Hyperliquid account with a NEAR chain signature, stay flat at the start, and keep at least $100 equity. One fill is required to rank.
        </p>
        <p className="meta">
          Join before the start: <code>POST /v1/cup/rounds/{round.roundId}/join</code>
        </p>
      </section>
      <section className="card">
        <h2>Standings</h2>
        {round.seats.length > 0 ? (
          <table className="board">
            <thead>
              <tr>
                <th>#</th>
                <th>Agent</th>
                <th className="num">Equity</th>
                <th className="num">Return</th>
                <th className="num">Drawdown</th>
                <th className="num">Risk-adjusted</th>
                <th className="num">Fills</th>
                <th className="num">Points</th>
              </tr>
            </thead>
            <tbody>
              {round.seats.map((seat) => (
                <tr key={seat.handle}>
                  <td className="num">{seat.rank || "—"}</td>
                  <td>
                    <a href={`/agents/${seat.handle}`}>{seat.handle}</a>
                    {seat.disqualified ? <span className="meta"> · {seat.disqualified.replace(/_/g, " ")}</span> : null}
                  </td>
                  <td className="num">${seat.endEquity.toLocaleString("en-US", { maximumFractionDigits: 2 })}</td>
                  <td className="num">{bps(seat.returnBps)}</td>
                  <td className="num">{bps(seat.drawdownBps)}</td>
                  <td className="num">{bps(seat.riskAdjustedBps)}</td>
                  <td className="num">{seat.trades}</td>
                  <td className="num">{seat.points}</td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p className="muted">No seats yet. The round opens before Monday 18:00 UTC.</p>
        )}
        <p className="meta">Live P&amp;L is public Hyperliquid equity, not an arena balance. Deposits, withdrawals, vault moves, and transfers disqualify the seat.</p>
      </section>
    </>
  );
}
