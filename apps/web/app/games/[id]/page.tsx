"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import { Spectator } from "@/components/Spectator";
import { TrustMarks } from "@/components/TrustMarks";
import { loadGame } from "@/lib/api";
import { yoctoToNear, type GameCard } from "@/lib/protocol";

export default function GamePage() {
  const params = useParams<{ id: string }>();
  const id = decodeURIComponent(params.id);
  const [game, setGame] = useState<GameCard | null>(null);
  const [source, setSource] = useState<"relay" | "fixture">("fixture");
  const [missing, setMissing] = useState(false);

  useEffect(() => {
    let alive = true;
    loadGame(id).then((result) => {
      if (!alive) return;
      setGame(result.game);
      setSource(result.source);
      setMissing(result.game === null);
    });
    return () => {
      alive = false;
    };
  }, [id]);

  if (missing) {
    return (
      <main>
        <h1>No such game</h1>
        <p className="lede">The relay and the local fixture both missed {id}.</p>
      </main>
    );
  }
  if (!game) return <main><p className="note">Loading game…</p></main>;

  return (
    <main>
      <h1>{game.id}</h1>
      <p className="lede">
        Live view of {game.rules.template_id}. The feed is a relay of events, not the chain. Creator prose is untrusted data.
      </p>
      {source === "fixture" && <p className="banner">Local replay of the Heist Night fixture. It is not chain data.</p>}
      <article className="card">
        <div className="row">
          <span>{yoctoToNear(game.rules.pot_yocto)} NEAR pot</span>
          <span className="meta">{game.rules.max_players} players · {game.rules.duration_seconds}s · {game.rules.round_count} rounds</span>
        </div>
        <p className="meta">creator {game.creator}</p>
        <p className="meta">pin {game.game_md_hash}</p>
        <TrustMarks trust={game.trust} />
      </article>
      <Spectator gameId={game.id} source={source} />
    </main>
  );
}
