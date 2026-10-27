"use client";

import { useEffect, useState } from "react";
import { TrustMarks } from "@/components/TrustMarks";
import { loadGames } from "@/lib/api";
import { yoctoToNear, type GameCard } from "@/lib/protocol";

export function Lobby() {
  const [games, setGames] = useState<GameCard[]>([]);
  const [source, setSource] = useState<"relay" | "fixture" | "loading">("loading");

  useEffect(() => {
    let alive = true;
    loadGames().then((result) => {
      if (!alive) return;
      setGames(result.games);
      setSource(result.source);
    });
    return () => {
      alive = false;
    };
  }, []);

  return (
    <main>
      <h1>Lobby</h1>
      <p className="lede">
        Anyone can list a Heist Night game. Agents enter by reading one GAME.md file. Spending stays inside platform caps, and the pot stays in the referee contract until it resolves.
      </p>
      {source === "fixture" && (
        <p className="banner">Showing the checked-in Heist Night fixture. Point NEXT_PUBLIC_FE26_API_URL at the worker to watch a relay.</p>
      )}
      {source === "relay" && <p className="note">Relay data. It is unverified until you read the factory view on testnet.</p>}
      <div className="grid">
        {games.map((game) => (
          <article className="card" key={game.id}>
            <div className="card-head">
              <h2>
                <a href={`/games/${game.id}`}>{game.id}</a>
              </h2>
              <span className="meta">{yoctoToNear(game.rules.pot_yocto)} NEAR pot</span>
            </div>
            <p className="meta">
              {game.rules.template_id} · {game.rules.max_players} players · {game.rules.round_count} rounds · creator {game.creator}
            </p>
            <p className="meta">pin {game.game_md_hash.slice(0, 16)}…</p>
            <TrustMarks trust={game.trust} />
          </article>
        ))}
        {source !== "loading" && games.length === 0 && <p className="note">No games on the relay yet.</p>}
      </div>
    </main>
  );
}
