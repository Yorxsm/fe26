import { NextResponse } from "next/server";

async function digest(value: string) {
  const buf = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
  return [...new Uint8Array(buf)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

export async function GET(request: Request) {
  const token = process.env.ADMIN_TOKEN;
  if (!token) return new NextResponse(null, { status: 404 });
  const cookie = request.headers
    .get("cookie")
    ?.split(";")
    .map((part) => part.trim())
    .find((part) => part.startsWith("fe2vi_admin="))
    ?.slice("fe2vi_admin=".length);
  if (!cookie || cookie !== (await digest(token))) return new NextResponse(null, { status: 404 });
  const started = Date.now();
  const health = await fetch("https://api.fe2vi.space/v1/stats", { cache: "no-store" }).catch(() => null);
  const latency = Date.now() - started;
  const detail = await fetch("https://api.fe2vi.space/v1/admin/status", {
    headers: { authorization: `Bearer ${token}` },
    cache: "no-store",
  }).catch(() => null);
  const body = detail && detail.ok ? await detail.json().catch(() => null) : null;
  return NextResponse.json({
    api: { ok: Boolean(health?.ok), status: health?.status ?? 0, latency_ms: latency },
    cron_last: body?.cron_last ?? null,
    errors: body?.errors ?? null,
    requests: body?.requests ?? null,
    durable_object: body?.durable_object ?? null,
  });
}
