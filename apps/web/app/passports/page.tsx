"use client";

import { useEffect, useMemo, useState } from "react";
import { apiBase } from "@/lib/protocol";
import { PlayerFigure } from "@/components/PlayerFigure";
import { accountFromStorage, priceFor, recordLine, splitByOwner, type PassportRow } from "@/lib/passports";

type Listing = { tokenId?: string; priceNear?: string | null; listed?: boolean };

export default function PassportsPage() {
  const [rows, setRows] = useState<PassportRow[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [account, setAccount] = useState("");
  const [connected, setConnected] = useState(false);
  const [tab, setTab] = useState<"owned" | "other">("other");

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
            const next = event.target.value;
            setAccount(next);
            if (!next.trim()) setTab("other");
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
        {shown.map((row) => {
          const record = recordLine(row.career);
          return (
            <a key={`${row.contract}:${row.tokenId}`} className="card passport-card" href={`/passports/${encodeURIComponent(row.tokenId)}`}>
              <PlayerFigure tokenId={row.tokenId} tier={row.tier} />
              <h2>{row.tokenId}</h2>
              {row.tier ? <p className="meta">{row.tier}</p> : null}
              {typeof row.career?.xp === "number" ? <p>{row.career.xp} XP</p> : null}
              {record ? <p className="meta">{record}</p> : null}
              {row.priceNear ? <p className="meta">Listed at {row.priceNear} NEAR</p> : null}
            </a>
          );
        })}
      </div>
    </main>
  );
}
