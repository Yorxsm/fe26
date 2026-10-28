"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import { QuestPing } from "@/components/QuestPing";
import { CountdownChip, EventBanner, HeistBoard, QuoteBoard, ScheduleList } from "@/components/SpectatorBoards";
import { Spectator } from "@/components/Spectator";
import { EmptyState, ErrorState, LoadingState } from "@/components/States";
import { TrustMarks } from "@/components/TrustMarks";
import { loadGame } from "@/lib/api";
import type { GameCard } from "@/lib/protocol";

const seats = [
  { account: "demo.registry.testnet", action: "scout", revoked: false },
  { account: "rival.registry.testnet", action: "hide", revoked: false },
];

export default function GamePage() {
  const params = useParams<{ id: string }>();
  const id = decodeURIComponent(params.id);
  const [game, setGame] = useState<GameCard | null>(null);
  const [source, setSource] = useState<"relay" | "fixture">("fixture");
  const [phase, setPhase] = useState<"loading" | "ready" | "missing" | "error">("loading");

  useEffect(() => {
    let alive = true;
    loadGame(id)
      .then((result) => {
        if (!alive) return;
        setGame(result.game);
        setSource(result.source);
        setPhase(result.game ? "ready" : "missing");
      })
      .catch(() => {
        if (alive) setPhase("error");
      });
    return () => {
      alive = false;
    };
  }, [id]);

  if (phase === "loading") {
    return (
      <main>
        <LoadingState label="the game" />
      </main>
    );
  }
  if (phase === "error") {
    return (
      <main>
        <ErrorState title="Game feed failed" detail="The page could not read the relay. Try the lobby again." />
      </main>
    );
  }
  if (phase === "missing" || !game) {
    return (
      <main>
        <EmptyState title="No such game" detail={`${id} is not on the relay or in the local fixture.`} />
      </main>
    );
  }

  const trading = game.rules.template_id === "trading-cup";
  const rounds = Array.from({ length: game.rules.round_count }, (_, index) => index);

  return (
    <main className="board">
      {game.rules.template_id === "heist-night" && <QuestPing quest="heist" />}
      <h1>{game.id}</h1>
      <div className="strip">
        <span>testnet</span>
        <span>{source === "fixture" ? "fixture replay" : "relay"}</span>
        <span>{game.verified_events ? `${game.verified_events} chain events` : "no chain verification yet"}</span>
        <span>round 2 now</span>
      </div>
      <TrustMarks trust={game.trust} templateId={game.rules.template_id} />
      <ol className="timeline" aria-label="Rounds">
        {rounds.map((round) => (
          <li key={round} className={round === 1 ? "now" : undefined}>
            Round {round + 1}
            <div>{round === 1 ? "now" : round < 1 ? "closed" : "later"}</div>
          </li>
        ))}
      </ol>
      <div className="arena">
        <div className="seats">
          {seats.map((seat) => (
            <article className={seat.revoked ? "seat seat-revoked" : "seat"} key={seat.account}>
              <h2 className="addr">{seat.account}</h2>
              <p>{trading ? "waiting on the tape" : `last action ${seat.action}`}</p>
              <p className="num">{seat.revoked ? "revoked" : "active"}</p>
            </article>
          ))}
        </div>
        <Spectator gameId={game.id} source={source} />
      </div>
      <CountdownChip />
      <EventBanner room={trading ? "cup-stadium" : "vault-quarter"} />
      <ScheduleList />
      <p>
        <a href={trading ? "/leaderboard?room=cup" : "/leaderboard?room=vault"}>Room leaderboard</a>
      </p>
      {trading ? <QuoteBoard /> : <HeistBoard />}
      <p className="muted">You are watching. Agents act from GAME.md. This page does not send a transaction.</p>
    </main>
  );
}
