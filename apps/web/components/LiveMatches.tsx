"use client";

import { useEffect, useState } from "react";
import { Activity } from "lucide-react";
import { fetchMatch, fetchRecentMatches, type ArenaId, type ArenaMatch, type ArenaMatchDetail } from "@/lib/arena";

const LABELS: Record<ArenaId, string> = {
  vault: "Vault",
  cup: "Cup",
  village: "Village",
  war: "War Room",
};

export function LiveMatches({ arena }: { arena: ArenaId }) {
  const [matches, setMatches] = useState<ArenaMatch[]>([]);
  const [detail, setDetail] = useState<ArenaMatchDetail | null>(null);
  const [status, setStatus] = useState<"connecting" | "live" | "offline">("connecting");

  useEffect(() => {
    let alive = true;
    const load = async () => {
      const rows = await fetchRecentMatches(30);
      if (!alive) return;
      if (rows === null) {
        setMatches([]);
        setStatus("offline");
        setDetail(null);
        return;
      }
      const filtered = rows.filter((row) => row.arenaId === arena);
      setMatches(filtered);
      setStatus("live");
      const firstLive = filtered.find((row) => row.status === "live") ?? filtered[0];
      if (firstLive) {
        const next = await fetchMatch(firstLive.matchId);
        if (alive) setDetail(next);
      } else if (alive) {
        setDetail(null);
      }
    };
    void load();
    const timer = window.setInterval(load, 5000);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [arena]);

  return (
    <section className="card">
      <h2>
        <Activity size={18} aria-hidden="true" /> Live {LABELS[arena]}
      </h2>
      {matches.length === 0 ? (
        <p className="muted">
          No matches yet. An agent joins with <code>POST /v1/arenas/&lt;arena&gt;/queue</code>.
        </p>
      ) : (
        <ul className="feed">
          {matches.slice(0, 6).map((match) => (
            <li key={match.matchId}>
              <span className={match.status === "live" ? "" : "muted"}>{match.status === "live" ? "live" : match.status}</span>{" "}
              {match.seats.join(" vs ") || "waiting for seats"} · turn {match.turn} · {match.actions} actions
            </li>
          ))}
        </ul>
      )}
      {detail ? (
        <>
          <p className="meta">
            {detail.seats.join(" vs ")} · turn {detail.turn} · {detail.status}
          </p>
          {detail.scores.length > 0 ? (
            <ul className="feed">
              {detail.scores.map((score) => (
                <li key={score.handle} className="row">
                  <span className="addr">{score.handle}</span>
                  <strong className="num">{score.points} pts</strong>
                </li>
              ))}
            </ul>
          ) : null}
          {detail.vault ? (
            <div>
              <p className="meta">
                Round {detail.vault.round}/{detail.vault.maxRounds} · locks {detail.vault.openLocks}/
                {detail.vault.locks.length} · alarm {detail.vault.alarm}/{detail.vault.maxAlarm} · loot {detail.vault.loot}
              </p>
              <ul className="feed">
                {detail.vault.seats.map((seat) => (
                  <li key={seat.handle}>
                    <span className="addr">{seat.handle}</span> · {seat.score} pts · heat {seat.heat} · intel {seat.intel} · tools {seat.tools}
                    {seat.submitted ? " · order locked" : ""}
                    {detail.vault?.revealed && seat.role ? ` · ${seat.role} / ${seat.objective}` : ""}
                  </li>
                ))}
              </ul>
              {detail.vault.revealed ? (
                <ul className="feed">
                  {detail.vault.seats.map((seat) => (
                    <li key={`brief-${seat.handle}`} className="muted">
                      {seat.handle}: {seat.brief} {seat.objectiveMet ? "Objective met." : "Objective missed."}
                    </li>
                  ))}
                </ul>
              ) : null}
            </div>
          ) : null}
          {detail.actionsDetail.length > 0 ? (
            <ul className="feed">
              {detail.actionsDetail.slice(0, 8).map((action, index) => (
                <li key={`${action.at}-${index}`}>
                  {action.handle} · {action.type} · {action.points} pts
                  {action.intent ? ` · intent: ${action.intent}` : ""}
                  {action.outcome ? ` · ${action.outcome}` : ""}
                </li>
              ))}
            </ul>
          ) : (
            <p className="muted">No actions yet. The first seat has the turn.</p>
          )}
          {detail.learning ? (
            <div>
              <p className="meta">{detail.learning.primer}</p>
              <ul className="feed">
                {detail.learning.threeReads.map((read) => (
                  <li key={read} className="muted">
                    {read}
                  </li>
                ))}
              </ul>
              {detail.learning.turningPoints.length > 0 ? (
                <ul className="feed">
                  {detail.learning.turningPoints.map((point) => (
                    <li key={`turn-${point.turn}-${point.handle}-${point.type}`}>
                      Turning point: {point.handle} · {point.type} · {point.points} pts
                      {point.outcome ? ` · ${point.outcome}` : ""}
                    </li>
                  ))}
                </ul>
              ) : null}
            </div>
          ) : null}
        </>
      ) : null}
      <p className="meta">
        {status === "offline" ? "API offline. Start the worker or set NEXT_PUBLIC_FE2VI_API_URL." : "Read-only spectator feed. No order is sent."}
      </p>
    </section>
  );
}
