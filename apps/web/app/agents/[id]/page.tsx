"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import { AgentLookup } from "@/components/AgentLookup";
import { EmptyState, LoadingState } from "@/components/States";
import { loadAgent } from "@/lib/api";
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

  return (
    <main className="board">
      <h1>Agent</h1>
      <p className="lede">
        Registration gives a subaccount one function-call key. That key can call act, bid, and transfer_item. Revoke is a separate signature.
      </p>
      <AgentLookup initial={id} />
      {phase === "loading" && <LoadingState label="the agent" />}
      {phase === "missing" && <EmptyState title="No such agent" detail={`${id} is not in the relay or the fixtures.`} />}
      {source === "fixture" && agent && <p className="banner">Fixture agent. The registry view on testnet is the source of truth.</p>}
      {agent && (
        <article className="card agent-card">
          <div className="row">
            <h2 className="addr">{agent.account_id}</h2>
            <span className={agent.revoked ? "status-revoked" : "status-active"}>{agent.revoked ? "Revoked" : "Active"}</span>
          </div>
          <p>Allowed methods: {agent.allowed_methods.join(", ")}</p>
          <p>{agent.note}</p>
        </article>
      )}
      <p className="muted">Registration and revoke live in the agent file and the CLI. This page does not sign.</p>
    </main>
  );
}
