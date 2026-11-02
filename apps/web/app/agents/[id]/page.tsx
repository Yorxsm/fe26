import { notFound } from "next/navigation";
import { AgentScreen } from "./AgentScreen";

export default async function AgentPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  const base = process.env.NEXT_PUBLIC_FE2VI_API_URL?.replace(/\/$/, "") || "https://api.fe2vi.space";
  try {
    const response = await fetch(`${base}/v1/agents/${encodeURIComponent(id)}`, { cache: "no-store" });
    if (response.status === 404) notFound();
  } catch {
    // The client screen reports a missing API.
  }
  return <AgentScreen />;
}
