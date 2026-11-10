"use client";

import { useEffect, useMemo, useState } from "react";
import { apiBase } from "@/lib/protocol";
import { accountFromStorage, priceFor, splitByOwner, type PassportRow } from "@/lib/passports";

type Listing = { tokenId?: string; priceNear?: string | null; listed?: boolean };

function careerLine(row: PassportRow): string {
  const career = row.career;
  if (!career) return "No career on this token.";
  const parts = [
    row.tier,
    career.xp === undefined ? null : `${career.xp} xp`,
    career.wins === undefined ? null : `${career.wins} wins`,
    career.matches === undefined ? null : `${career.matches} matches`,
  ].filter(Boolean);
  return parts.length ? parts.join(" · ") : "No career on this token.";
}

export default function PassportsPage() {
  const [rows, setRows] = useState<PassportRow[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [account, setAccount] = useState("");
  const [connected, setConnected] = useState(false);
  const [tab, setTab] = useState<"owned" | "other">("owned");

  useEffect(() => {
    const entries: Array<[string, string | null]> = [];
    for (let index = 0; index < localStorage.length; index += 1) {
      const key = localStorage.key(index);
      if (key) entries.push([key, localStorage.getItem(key)]);
    }
    const wallet = accountFromStorage(entries);
    if (wallet) {
      setAccount(wallet);
      setConnected(true);
    }
    let alive = true;
    const base = apiBase();
    Promise.all([
      fetch(`${base}/v1/passports`, { cache: "no-store" }),
      fetch(`${base}/v1/market/listings`, { cache: "no-store" }).catch(() => null),
    ])
      .then(async ([passportResponse, listingResponse]) => {
        if (!alive) return;
        if (!passportResponse.ok) {
          setFailed(true);
          setRows([]);
          return;
        }
        const body = (await passportResponse.json()) as { passports?: PassportRow[] };
        const listings: Listing[] = [];
        if (listingResponse && listingResponse.ok) {
          const listed = (await listingResponse.json()) as { listings?: Listing[]; passports?: Listing[] };
          listings.push(...(listed.listings ?? []), ...(listed.passports ?? []));
        }
        const next = (body.passports ?? []).map((row) => ({
          ...row,
          priceNear: priceFor(row.tokenId, listings),
        }));
        setRows(next);
      })
      .catch(() => {
        if (!alive) return;
        setFailed(true);
        setRows([]);
      });
    return () => {
      alive = false;
    };
  }, []);

  const split = useMemo(() => splitByOwner(rows ?? [], account), [rows, account]);
  const shown = tab === "owned" ? split.owned : split.other;

  return (
    <main className="board">
      <h1>Passports</h1>
      <p className="lede">Testnet tokens from the arena passport and the recorder passport. The playbook stays off this page.</p>
      <label className="passport-account">
        <span>{connected ? "Connected testnet account" : "Testnet account"}</span>
        <input
          value={account}
          onChange={(event) => {
            setConnected(false);
            setAccount(event.target.value);
          }}
          placeholder="name.testnet"
          autoComplete="off"
          spellCheck={false}
        />
      </label>
      <div className="passport-tabs" role="tablist" aria-label="Passport ownership">
        <button type="button" role="tab" aria-selected={tab === "owned"} onClick={() => setTab("owned")}>
          Owned ({split.owned.length})
        </button>
        <button type="button" role="tab" aria-selected={tab === "other"} onClick={() => setTab("other")}>
          Not owned ({split.other.length})
        </button>
      </div>
      {rows === null ? <p className="meta">Reading passports.</p> : null}
      {failed ? <p className="meta">The passport list did not load.</p> : null}
      {rows && !failed && shown.length === 0 ? (
        <p className="meta">
          {tab === "owned" && !account.trim()
            ? "Enter a testnet account, or connect a testnet wallet, to see what it owns."
            : "No passports in this section."}
        </p>
      ) : null}
      <div className="passport-grid">
        {shown.map((row) => (
          <a key={`${row.contract}:${row.tokenId}`} className="card passport-card" href={`/passports/${encodeURIComponent(row.tokenId)}`}>
            <h2>{row.tokenId}</h2>
            <p className="meta">{row.owner || "Owner unavailable"}</p>
            <p>{careerLine(row)}</p>
            <p className="meta">
              {row.strategyVersion === null ? "No playbook version." : `Playbook v${row.strategyVersion}`}
              {row.strategyHash ? ` · ${row.strategyHash.slice(0, 12)}…` : ""}
            </p>
            <p className="meta">{row.priceNear ? `Listed at ${row.priceNear} NEAR` : "Not listed."}</p>
            <p className="meta">{row.contract}</p>
          </a>
        ))}
      </div>
    </main>
  );
}
