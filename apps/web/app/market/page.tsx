"use client";

import { useEffect, useMemo, useState } from "react";
import { AgentPortrait } from "@/components/SkinPreview";
import { accountFromStorage, listedOffer, recordLine, splitByOwner, tierFrame, type PassportRow } from "@/lib/passports";
import { fetchMarketData } from "@/lib/market";
import { buyHandoff, configuredEconomy } from "@/lib/nearBuy";
import { defaultSkin } from "@/lib/skin";
import { apiBase } from "@/lib/protocol";

type Offer = { priceNear: string; priceYocto: string | null };

export default function MarketPage() {
  const [rows, setRows] = useState<PassportRow[] | null>(null);
  const [offers, setOffers] = useState<Array<{ tokenId?: string; priceNear?: string | null; priceYocto?: string | null; listed?: boolean }>>([]);
  const [failed, setFailed] = useState(false);
  const [account, setAccount] = useState("");
  const [connected, setConnected] = useState(false);
  const [tab, setTab] = useState<"owned" | "other">("other");
  const [market, setMarket] = useState("");
  const [callback, setCallback] = useState("https://fe2vi.space/market");

  useEffect(() => {
    const entries: Array<[string, string | null]> = [];
    for (let index = 0; index < localStorage.length; index += 1) {
      const key = localStorage.key(index);
      if (key) entries.push([key, localStorage.getItem(key)]);
    }
    setCallback(`${window.location.origin}/market`);
    const wallet = accountFromStorage(entries);
    if (wallet) {
      setAccount(wallet);
      setConnected(true);
    }
    let alive = true;
    const base = apiBase();
    Promise.all([
      fetch(`${base}/v1/passports`, { cache: "no-store" }),
      fetchMarketData(),
    ])
      .then(async ([passportResponse, marketResult]) => {
        if (!alive) return;
        setMarket(marketResult.contract);
        const listings = [...marketResult.listings, ...marketResult.passports];
        setOffers(listings);
        if (!passportResponse.ok) {
          setFailed(true);
          setRows([]);
          return;
        }
        const body = (await passportResponse.json()) as { passports?: PassportRow[] };
        setRows(body.passports ?? []);
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
      <h1>Market</h1>
      <p className="lede">
        Passport NFTs from the testnet contracts. A price is shown only when the market returned an ask.
        {market ? <> Market: <code>{market}</code>.</> : null}
      </p>
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
          const offer: Offer | null = listedOffer(row.tokenId, offers);
          const handoff = offer?.priceYocto ? buyHandoff(configuredEconomy(), row.tokenId, offer.priceYocto, callback) : null;
          return (
            <article key={`${row.contract}:${row.tokenId}`} className="card passport-card" style={{ borderColor: tierFrame(row.tier) }}>
              <a href={`/passports/${encodeURIComponent(row.tokenId)}`}>
                <span className="passport-frame" style={{ borderColor: tierFrame(row.tier) }}>
                  <AgentPortrait traits={defaultSkin(row.tokenId)} />
                </span>
                <h2>{row.tokenId}</h2>
                {row.tier ? <p className="meta">{row.tier}</p> : null}
                {typeof row.career?.xp === "number" ? <p>{row.career.xp} XP</p> : null}
                {record ? <p className="meta">{record}</p> : null}
                <p className="meta">{offer ? `${offer.priceNear} NEAR` : "Not listed"}</p>
              </a>
              {handoff?.ok ? (
                <a className="btn btn-primary" href={handoff.login}>
                  Buy {offer?.priceNear} NEAR
                </a>
              ) : null}
            </article>
          );
        })}
      </div>
    </main>
  );
}
