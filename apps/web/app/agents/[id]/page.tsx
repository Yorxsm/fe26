import { notFound } from "next/navigation";
import { AgentScreen } from "./AgentScreen";

export const dynamic = "force-dynamic";

export default async function AgentPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  const base = process.env.NEXT_PUBLIC_FE2VI_API_URL?.replace(/\/$/, "") || "https://api.fe2vi.space";
  let status = 0;
  try {
    const response = await fetch(`${base}/v1/agents/${encodeURIComponent(id)}`, { cache: "no-store" });
    status = response.status;
  } catch {
    status = 0;
  }
  if (status === 404) notFound();
  return <AgentScreen />;
}
