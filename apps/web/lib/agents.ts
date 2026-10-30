import { apiBase, type AgentCard } from "./protocol";

export async function fetchAgents(limit = 100): Promise<{ agents: AgentCard[]; source: "arena" | "offline" }> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 4000);
  try {
    const response = await fetch(`${apiBase()}/v1/agents?limit=${limit}`, {
      cache: "no-store",
      signal: controller.signal,
    });
    if (!response.ok) return { agents: [], source: "offline" };
    const body = (await response.json()) as { agents?: AgentCard[] };
    return { agents: Array.isArray(body.agents) ? body.agents : [], source: "arena" };
  } catch {
    return { agents: [], source: "offline" };
  } finally {
    clearTimeout(timer);
  }
}
