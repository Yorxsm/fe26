"use client";

import { useEffect, useState } from "react";
import { EmptyState, ErrorState, LoadingState } from "@/components/States";
import { TrustMarks } from "@/components/TrustMarks";
import { loadGames } from "@/lib/api";
import { yoctoToNear, type GameCard } from "@/lib/protocol";

export function Lobby({ panel = false }: { panel?: boolean }) {
  const [games, setGames] = useState<GameCard[]>([]);
  const [source, setSource] = useState<"relay" | "fixture" | "loading" | "error">("loading");

  useEffect(() => {
    let alive = true;
    loadGames()
      .then((result) => {
        if (!alive) return;
        setGames(result.games);
        setSource(result.source);
      })
      .catch(() => {
        if (alive) setSource("error");
      });
    return () => {
      alive = false;
    };
  }, []);

  const Root = panel ? "div" : "main";
  return (
    <Root className={panel ? "hud-lobby" : "board"}>
      {panel ? <p className="hud-tab">Games</p> : <h1>Lobby</h1>}
      <p className="lede">
        A live board of games any agent can enter by reading one GAME.md. The pot stays in the referee. Caps are platform law.
      </p>
      <div className="strip" aria-label="Status">
        <span>testnet</span>
        <span>{source === "fixture" ? "fixture replay" : source === "relay" ? "relay, unverified until chain read" : source}</span>
      </div>
      {source === "loading" && <LoadingState label="the lobby" />}
      {source === "error" && <ErrorState title="Lobby unavailable" detail="The relay did not answer. Refresh, or leave the API URL unset to see the fixture." />}
      {source !== "loading" && source !== "error" && games.length === 0 && (
        <EmptyState title="No games listed" detail="The relay is empty. Create a Heist Night or Trading Cup game when you are ready to sign on testnet." />
      )}
      <div className="grid">
        {games.map((game) => (
          <article className="card" key={game.id}>
            <div className="card-head">
              <h2>
                <a href={`/games/${game.id}`}>{game.id}</a>
              </h2>
              <span className="meta">{yoctoToNear(game.rules.pot_yocto)} NEAR escrowed</span>
            </div>
            <p className="meta">
              {game.rules.template_id} · {game.rules.max_players} seats · {game.rules.round_count} rounds · {game.creator}
            </p>
            <p className="meta">pin {game.game_md_hash}</p>
            <TrustMarks trust={game.trust} templateId={game.rules.template_id} />
          </article>
        ))}
      </div>
    </Root>
  );
}
