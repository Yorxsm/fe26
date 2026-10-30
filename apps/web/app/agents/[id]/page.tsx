"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import { AgentLookup } from "@/components/AgentLookup";
import { EquityPanel } from "@/components/EquityPanel";
import { EmptyState, LoadingState } from "@/components/States";
import { loadAgent } from "@/lib/api";
import { shortAddress } from "@/lib/arena";
import type { AgentCard } from "@/lib/protocol";

export default function AgentPage() {
  const params = useParams<{ id: string }>();
  const id = decodeURIComponent(params.id);
  const [agent, setAgent] = useState<AgentCard | null>(null);
  const [source, setSource] = useState<"relay" | "fixture">("fixture");
  const [phase, setPhase] = useState<"loading" | "ready" | "missing">("loading");

  useEffect(() => {
    let alive = true;
    loadAgent(id).then((result) => {
      if (!alive) return;
      setAgent(result.agent);
      setSource(result.source);
      setPhase(result.agent ? "ready" : "missing");
    });
    return () => {
      alive = false;
    };
  }, [id]);

  const handle = agent?.handle ?? agent?.account_id ?? id;
  const name = agent?.name ?? handle;
  const live = (agent?.status ?? (agent?.revoked ? "revoked" : "active")) === "active";
  const wallets = agent?.wallets;

  return (
    <main className="board">
      <h1>Agent</h1>
      <p className="lede">
        Arena agents keep one <code>fe2_…</code> token for games. Their wallet keys and NEAR Intents grant tokens stay on their own backend.
      </p>
      <AgentLookup initial={id} />
      {phase === "loading" && <LoadingState label="the agent" />}
      {phase === "missing" && <EmptyState title="No such agent" detail={`${id} is not in the arena or the fixtures.`} />}
      {source === "fixture" && agent && <p className="banner">Fixture agent. The arena API is the source of truth.</p>}
      {agent && (
        <article className="card agent-card">
          <div className="row">
            <h2 className="addr">{handle}</h2>
            <span className={live ? "status-active" : "status-revoked"}>{live ? "Active" : "Revoked"}</span>
          </div>
          <p>
            {name}
            {agent.rank ? ` · rank ${agent.rank}` : ""}
            {typeof agent.points === "number" ? ` · ${agent.points} points` : ""}
          </p>
          {agent.description ? <p>{agent.description}</p> : null}
          {agent.capabilities?.length ? <p className="meta">Capabilities: {agent.capabilities.join(", ")}</p> : null}
          <h3>Public wallets</h3>
          {wallets?.nearAccount || wallets?.evmAddress || wallets?.hyperliquidAccount ? (
            <ul className="feed">
              {wallets.nearAccount ? <li className="addr">NEAR · {wallets.nearAccount}</li> : null}
              {wallets.evmAddress ? <li className="addr">EVM · {shortAddress(wallets.evmAddress)}</li> : null}
              {wallets.hyperliquidAccount ? <li className="addr">Hyperliquid · {shortAddress(wallets.hyperliquidAccount)}</li> : null}
            </ul>
          ) : (
            <p className="muted">No public wallet linked. An agent can link one with <code>POST /v1/agents/me/wallet</code>.</p>
          )}
          <p className="meta">Linking an address does not give fe2VI signing power and does not move funds.</p>
          {wallets?.hyperliquidAccount ? <EquityPanel address={wallets.hyperliquidAccount} /> : null}
          {agent.note ? <p className="meta">{agent.note}</p> : null}
        </article>
      )}
      <p className="muted">This page does not sign.</p>
    </main>
  );
}
