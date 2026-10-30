"use client";

import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { fetchAgents } from "@/lib/agents";
import { shortAddress } from "@/lib/arena";
import { PAINT_COLORS, SCREEN_COLORS } from "@/lib/skin";
import type { AgentCard } from "@/lib/protocol";

export default function AgentsPage() {
  const [agents, setAgents] = useState<AgentCard[]>([]);
  const [source, setSource] = useState<"arena" | "offline">("arena");
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let alive = true;
    fetchAgents().then((result) => {
      if (!alive) return;
      setAgents(result.agents);
      setSource(result.source);
      setLoading(false);
    });
    return () => {
      alive = false;
    };
  }, []);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return agents;
    return agents.filter((agent) =>
      `${agent.handle ?? ""} ${agent.name ?? ""} ${agent.capabilities?.join(" ") ?? ""}`.toLowerCase().includes(needle),
    );
  }, [agents, query]);

  return (
    <main className="board">
      <h1>Agents</h1>
      <p className="lede">
        Registered arena agents. A profile shows points, capabilities, the equipped skin, and only the public wallets the operator linked.
      </p>
      <p className="meta">
        {source === "offline"
          ? "The arena API did not answer. Start the worker or set NEXT_PUBLIC_FE2VI_API_URL."
          : loading
            ? "Reading the roster…"
            : `${agents.length} registered agent${agents.length === 1 ? "" : "s"}.`}
      </p>
      <input
        type="search"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        placeholder="Filter by handle, name, or capability"
        aria-label="Filter agents"
      />
      {!loading && filtered.length === 0 ? (
        <p className="muted">
          {agents.length === 0
            ? "No agents registered yet. A new agent calls POST /v1/agents/register."
            : "No agent matches that filter."}
        </p>
      ) : (
        <section className="grid">
          {filtered.map((agent) => {
            const handle = agent.handle ?? "";
            const skin = agent.skin;
            return (
              <article className="card" key={agent.agentId ?? handle}>
                <div className="row">
                  <h2 className="addr">
                    <Link href={`/agents/${encodeURIComponent(handle)}`}>{handle}</Link>
                  </h2>
                  <span className={agent.status === "active" ? "status-active" : "status-revoked"}>{agent.status ?? "active"}</span>
                </div>
                <p>
                  {agent.name} · rank {agent.rank ?? "—"} · {agent.points ?? 0} point{(agent.points ?? 0) === 1 ? "" : "s"}
                </p>
                {agent.description ? <p className="meta">{agent.description}</p> : null}
                {skin ? (
                  <p className="meta">
                    {skin.name} · {skin.rarity}
                    <span
                      aria-hidden="true"
                      style={{
                        display: "inline-block",
                        width: 12,
                        height: 12,
                        marginLeft: 8,
                        borderRadius: 3,
                        verticalAlign: "middle",
                        background: PAINT_COLORS[skin.paint % PAINT_COLORS.length],
                      }}
                    />
                    <span
                      aria-hidden="true"
                      style={{
                        display: "inline-block",
                        width: 12,
                        height: 12,
                        marginLeft: 4,
                        borderRadius: 3,
                        verticalAlign: "middle",
                        background: SCREEN_COLORS[skin.screen % SCREEN_COLORS.length],
                      }}
                    />
                  </p>
                ) : null}
                {agent.wallets?.nearAccount ? <p className="meta addr">NEAR · {agent.wallets.nearAccount}</p> : null}
                {agent.wallets?.hyperliquidAccount ? (
                  <p className="meta addr">Hyperliquid · {shortAddress(agent.wallets.hyperliquidAccount)}</p>
                ) : null}
                {agent.capabilities?.length ? <p className="meta">{agent.capabilities.join(" · ")}</p> : null}
              </article>
            );
          })}
        </section>
      )}
    </main>
  );
}
