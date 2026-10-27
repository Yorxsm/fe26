"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import { QuestPing } from "@/components/QuestPing";
import { SignPanel } from "@/components/SignPanel";
import { Spectator } from "@/components/Spectator";
import { EmptyState, ErrorState, LoadingState } from "@/components/States";
import { TrustMarks } from "@/components/TrustMarks";
import { loadGame } from "@/lib/api";
import { yoctoToNear, type GameCard } from "@/lib/protocol";
import { PLACEHOLDER_CONTRACTS } from "@/lib/wallet";

const seats = [
  { account: "demo.registry.testnet", action: "scout", points: "1", spent: "0.125 NEAR", revoked: false },
  { account: "rival.registry.testnet", action: "hide", points: "1", spent: "0.125 NEAR", revoked: false },
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

  const potNear = Number(yoctoToNear(game.rules.pot_yocto));
  const fill = Math.max(4, Math.min(100, (potNear / 50) * 100));
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
      <section className="card">
        <div className="row">
          <h2>Pot</h2>
          <strong>{yoctoToNear(game.rules.pot_yocto)} NEAR escrowed</strong>
        </div>
        <div className="meter">
          <div className="meter-track" aria-hidden="true">
            <div className="meter-fill" style={{ ["--fill" as string]: `${fill}%` }} />
          </div>
          <p className="meta">Escrowed against the 50 NEAR platform pot cap. The referee holds the deposit until resolve.</p>
        </div>
      </section>
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
              <h2>{seat.account}</h2>
              <p>{trading ? "waiting on the tape" : `last action ${seat.action}`}</p>
              <p className="meta">
                {seat.points} points · spent {seat.spent} · {seat.revoked ? "revoked" : "active"}
              </p>
            </article>
          ))}
        </div>
        <Spectator gameId={game.id} source={source} />
      </div>
      <SignPanel
        title={trading ? "Sign long or short" : "Sign act"}
        call={{
          receiverId: PLACEHOLDER_CONTRACTS.heist,
          methodName: "act",
          args: trading
            ? { game_id: game.id, action: "long", target: null, stake_yocto: game.rules.per_action_cap_yocto }
            : { game_id: game.id, action: "scout", target: null, stake_yocto: String(BigInt(game.rules.per_action_cap_yocto) / 4n) },
          deposit: "1",
        }}
        idle="A connected testnet wallet can submit act. The scoped key is still the agent path."
      />
    </main>
  );
}
