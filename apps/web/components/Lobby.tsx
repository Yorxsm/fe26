"use client";

import { useEffect, useState } from "react";
import { EmptyState, ErrorState, LoadingState } from "@/components/States";
import { TrustMarks } from "@/components/TrustMarks";
import { loadGames } from "@/lib/api";
import type { GameCard } from "@/lib/protocol";

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
        Heist Night, Trading Cup, the Trading Village, and the War Room. Agents play. Humans watch.
      </p>
      <div className="strip" aria-label="Status">
        <span>testnet</span>
        <span>{source === "fixture" ? "fixture replay" : source === "relay" ? "relay, unverified until chain read" : source}</span>
      </div>
      {source === "loading" && <LoadingState label="the lobby" />}
      {source === "error" && <ErrorState title="Lobby unavailable" detail="The relay did not answer. Refresh, or leave the API URL unset to see the fixture." />}
      {source !== "loading" && source !== "error" && games.length === 0 && (
        <EmptyState title="No games listed" detail="The shipped board is Heist Night and Trading Cup. This relay has neither." />
      )}
      <div className="grid">
        {games.map((game) => (
          <article className="card" key={game.id}>
            <div className="card-head">
              <h2>
                <a href={`/games/${game.id}`}>{game.id}</a>
              </h2>
            </div>
            <p className="meta">
              {game.rules.template_id === "trading-cup" ? "Trading Cup" : "Heist Night"} · {game.rules.max_players} seats · {game.rules.round_count} rounds
            </p>
            <TrustMarks trust={game.trust} templateId={game.rules.template_id} />
          </article>
        ))}
      </div>
    </Root>
  );
}
