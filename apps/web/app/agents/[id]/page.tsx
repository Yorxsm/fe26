"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import { AgentLookup } from "@/components/AgentLookup";
import { SignPanel } from "@/components/SignPanel";
import { EmptyState, LoadingState } from "@/components/States";
import { loadAgent } from "@/lib/api";
import { defaultSkin } from "@/lib/skin";
import { yoctoToNear, type AgentCard } from "@/lib/protocol";
import { PLACEHOLDER_CONTRACTS } from "@/lib/wallet";

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
        <article className="card">
          <div className="row">
            <h2 className="addr">{agent.account_id}</h2>
            <span className={agent.revoked ? "status-revoked" : "status-active"}>{agent.revoked ? "Revoked" : "Active"}</span>
          </div>
          <p>Allowed methods: {agent.allowed_methods.join(", ")}</p>
          <p className="num">Per-agent platform cap {yoctoToNear(agent.platform_caps.max_per_agent_yocto)} NEAR</p>
          <p className="num">Per-action platform cap {yoctoToNear(agent.platform_caps.max_per_action_yocto)} NEAR</p>
          <p className="num">
            Pot cap {yoctoToNear(agent.platform_caps.max_pot_yocto)} NEAR · {agent.platform_caps.max_players} players
          </p>
          <p>{agent.note}</p>
          <h3>Skin</h3>
          <p>
            Default look, free, from the account id. Paint {defaultSkin(agent.account_id).paint}, screen {defaultSkin(agent.account_id).screen}, eyes{" "}
            {defaultSkin(agent.account_id).eyes}, headwear {defaultSkin(agent.account_id).headwear}, face {defaultSkin(agent.account_id).face_item}, back{" "}
            {defaultSkin(agent.account_id).back_item}, emblem {defaultSkin(agent.account_id).emblem}, trail {defaultSkin(agent.account_id).trail}, role{" "}
            {defaultSkin(agent.account_id).role}.
          </p>
          <p className="meta">No minted skin is read here. A mint would be cosmetic only and would show beside this default.</p>
        </article>
      )}
      <SignPanel
        title="Sign register"
        call={{
          receiverId: PLACEHOLDER_CONTRACTS.registry,
          methodName: "register_agent",
          args: { name: "demo", public_key: "ed25519:replace-with-your-agent-key" },
          deposit: "100000000000000000000000",
          gas: "200000000000000",
        }}
        idle="Connect a testnet wallet to register. The deposit is the gas balance forwarded to the subaccount."
      />
      <SignPanel
        title="Sign revoke"
        call={{
          receiverId: PLACEHOLDER_CONTRACTS.registry,
          methodName: "revoke",
          args: { agent: id },
          deposit: "1",
        }}
        idle="Revoke uses the registrant or owner key, not the scoped agent key."
      />
    </main>
  );
}
