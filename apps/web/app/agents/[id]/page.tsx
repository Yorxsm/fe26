"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import { AgentLookup } from "@/components/AgentLookup";
import { loadAgent } from "@/lib/api";
import { yoctoToNear, type AgentCard } from "@/lib/protocol";

export default function AgentPage() {
  const params = useParams<{ id: string }>();
  const id = decodeURIComponent(params.id);
  const [agent, setAgent] = useState<AgentCard | null>(null);
  const [source, setSource] = useState<"relay" | "fixture">("fixture");
  const [missing, setMissing] = useState(false);

  useEffect(() => {
    let alive = true;
    loadAgent(id).then((result) => {
      if (!alive) return;
      setAgent(result.agent);
      setSource(result.source);
      setMissing(result.agent === null);
    });
    return () => {
      alive = false;
    };
  }, [id]);

  return (
    <main>
      <h1>Agent</h1>
      <p className="lede">Registration gives a subaccount one function-call key. That key can call act, bid, and transfer_item. Revocation is enforced by the registry before a spend.</p>
      <AgentLookup initial={id} />
      {source === "fixture" && agent && <p className="banner">Fixture agent. The registry view on testnet is the source of truth.</p>}
      {missing && <p className="note">No agent named {id} in the relay or the fixtures.</p>}
      {agent && (
        <article className="card">
          <div className="row">
            <h2>{agent.account_id}</h2>
            <span className={agent.revoked ? "status-revoked" : "status-active"}>{agent.revoked ? "Revoked" : "Active"}</span>
          </div>
          <p>Allowed methods: {agent.allowed_methods.join(", ")}</p>
          <p className="meta">Per-agent platform cap {yoctoToNear(agent.platform_caps.max_per_agent_yocto)} NEAR</p>
          <p className="meta">Per-action platform cap {yoctoToNear(agent.platform_caps.max_per_action_yocto)} NEAR</p>
          <p className="meta">Pot cap {yoctoToNear(agent.platform_caps.max_pot_yocto)} NEAR · {agent.platform_caps.max_players} players</p>
          <p>{agent.note}</p>
        </article>
      )}
    </main>
  );
}
