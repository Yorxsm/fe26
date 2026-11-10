export const dynamic = "force-dynamic";

type PublicRecord = {
  status: string | null;
  ownerNear: string | null;
  count: number | null;
  updated: string | null;
  tier: string | null;
  hash: string | null;
  version: number | null;
  contract: string | null;
  xp: number | null;
};

const EMPTY: PublicRecord = {
  status: null,
  ownerNear: null,
  count: null,
  updated: null,
  tier: null,
  hash: null,
  version: null,
  contract: null,
  xp: null,
};

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
    tier: typeof row.tier === "string" ? row.tier : null,
    hash: typeof row.strategyHash === "string" ? row.strategyHash : null,
    version: typeof row.strategyVersion === "number" ? row.strategyVersion : null,
    contract: typeof (body as { contract?: unknown }).contract === "string" ? (body as { contract: string }).contract : null,
    xp: typeof (row.onchainCareer as { xp?: unknown } | undefined)?.xp === "number" ? (row.onchainCareer as { xp: number }).xp : null,
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
          {record.tier ? ` · ${record.tier}` : ""}
          {record.xp === null ? "" : ` · ${record.xp} xp`}
        </p>
        <p className="meta">
          {record.version === null ? "No on-chain playbook version." : `On-chain playbook v${record.version}.`}
          {record.hash ? ` ${record.hash}` : ""}
        </p>
        {record.contract ? <p className="meta">{record.contract}</p> : null}
      </section>
    </main>
  );
}
