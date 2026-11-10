import { AgentPortrait } from "@/components/SkinPreview";
import { passportDetail, tierFrame, type CareerNumbers } from "@/lib/passports";
import { defaultSkin } from "@/lib/skin";

export const dynamic = "force-dynamic";

function careerBits(career: CareerNumbers | null): Array<[string, number]> {
  if (!career) return [];
  const fields: Array<[string, number | undefined]> = [
    ["XP", career.xp],
    ["Wins", career.wins],
    ["Draws", career.draws],
    ["Losses", career.losses],
    ["Matches", career.matches],
  ];
  return fields.filter((entry): entry is [string, number] => typeof entry[1] === "number");
}

export default async function PassportPage({ params }: { params: Promise<{ tokenId: string }> }) {
  const { tokenId } = await params;
  const base = process.env.NEXT_PUBLIC_FE2VI_API_URL?.replace(/\/$/, "") || "https://api.fe2vi.space";
  let record = passportDetail(null);
  let note = "The public passport record did not load.";
  try {
    const response = await fetch(`${base}/v1/passports/${encodeURIComponent(tokenId)}`, { cache: "no-store" });
    if (response.ok) {
      record = passportDetail(await response.json());
      note = "Public passport record. The playbook stays with the bound owner.";
    } else if (response.status === 404) {
      note = "No passport with that token id is in the arena book.";
    }
  } catch {
    note = "The public passport record did not load.";
  }
  const career = careerBits(record.career);
  const onchain = careerBits(record.onchainCareer);

  return (
    <main className="board passport-detail">
      <h1>{tokenId}</h1>
      <p className="lede">{note}</p>
      <span className="passport-frame hero" style={{ borderColor: tierFrame(record.tier) }}>
        <AgentPortrait traits={defaultSkin(tokenId)} large />
      </span>
      <section className="card passport-facts">
        {record.owner ? <p>Owner {record.owner}</p> : null}
        {record.contract ? (
          <p>
            Contract{" "}
            {record.contractUrl ? (
              <a href={record.contractUrl}>{record.contract}</a>
            ) : (
              record.contract
            )}
          </p>
        ) : null}
        {record.boundAgent ? <p>Bound agent {record.boundAgent}</p> : null}
        {record.tier ? <p>Tier {record.tier}</p> : null}
        {career.map(([label, value]) => (
          <p key={label}>
            {label} {value}
          </p>
        ))}
        {onchain.length ? (
          <div>
            <h2>On-chain career</h2>
            {onchain.map(([label, value]) => (
              <p key={`chain-${label}`}>
                {label} {value}
              </p>
            ))}
          </div>
        ) : null}
        {record.strategyVersionCount !== null ? <p>Strategy versions {record.strategyVersionCount}</p> : null}
        {record.strategyHash ? <p className="meta">Playbook hash {record.strategyHash}</p> : null}
        {record.strategyUpdatedAt ? <p className="meta">Updated {record.strategyUpdatedAt}</p> : null}
      </section>
    </main>
  );
}
