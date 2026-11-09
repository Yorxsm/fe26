export const dynamic = "force-dynamic";

type PublicRecord = {
  status: string | null;
  ownerNear: string | null;
  count: number | null;
  updated: string | null;
};

const EMPTY: PublicRecord = { status: null, ownerNear: null, count: null, updated: null };

function publicRecord(body: unknown): PublicRecord {
  if (!body || typeof body !== "object") return EMPTY;
  const passport = (body as { passport?: unknown }).passport;
  if (!passport || typeof passport !== "object") return EMPTY;
  const row = passport as Record<string, unknown>;
  return {
    status: typeof row.status === "string" ? row.status : null,
    ownerNear: typeof row.ownerNear === "string" ? row.ownerNear : null,
    count: typeof row.strategyVersionCount === "number" ? row.strategyVersionCount : null,
    updated: typeof row.strategyUpdatedAt === "string" ? row.strategyUpdatedAt : null,
  };
}

export default async function PassportPage({ params }: { params: Promise<{ tokenId: string }> }) {
  const { tokenId } = await params;
  const base = process.env.NEXT_PUBLIC_FE2VI_API_URL?.replace(/\/$/, "") || "https://api.fe2vi.space";
  let record = EMPTY;
  let note = "The public passport record did not load.";
  try {
    const response = await fetch(`${base}/v1/passports/${encodeURIComponent(tokenId)}`, { cache: "no-store" });
    if (response.ok) {
      record = publicRecord(await response.json());
      note = "This page shows the public count only. The playbook stays with the bound owner.";
    } else if (response.status === 404) {
      note = "No passport with that token id is in the arena book.";
    }
  } catch {
    note = "The public passport record did not load.";
  }

  return (
    <main className="board">
      <h1>{tokenId}</h1>
      <p className="lede">{note}</p>
      <section className="card">
        <h2>Strategy</h2>
        <p>{record.count === null ? "No strategy version count is on this record." : `${record.count} versions.`}</p>
        <p className="meta">{record.updated ? `Last updated ${record.updated}.` : "No strategy update is recorded."}</p>
        <p className="meta">
          {record.status ?? "Status unavailable"}
          {record.ownerNear ? ` · ${record.ownerNear}` : ""}
        </p>
      </section>
    </main>
  );
}
