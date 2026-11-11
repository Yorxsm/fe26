"use client";

import { useEffect, useMemo, useState } from "react";
import { PortraitStill } from "@/components/PortraitStill";
import {
  accountFromStorage,
  listedOffer,
  marketStats,
  recordLine,
  onchainMedia,
  sortMarket,
  splitByOwner,
  tierFrame,
  type MarketSort,
  type PassportRow,
} from "@/lib/passports";
import { fetchMarketData } from "@/lib/market";
import { buyHandoff, configuredEconomy } from "@/lib/nearBuy";
import { defaultSkin } from "@/lib/skin";
import { apiBase } from "@/lib/protocol";

type OfferRow = { tokenId?: string; priceNear?: string | null; priceYocto?: string | null; listed?: boolean };
type Filter = "owned" | "other" | "listed";

export default function MarketPage() {
  const [rows, setRows] = useState<PassportRow[] | null>(null);
  const [offers, setOffers] = useState<OfferRow[]>([]);
  const [failed, setFailed] = useState(false);
  const [account, setAccount] = useState("");
  const [connected, setConnected] = useState(false);
  const [filter, setFilter] = useState<Filter>("other");
  const [sort, setSort] = useState<MarketSort>("id");
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
    Promise.all([fetch(`${base}/v1/passports`, { cache: "no-store" }), fetchMarketData()])
      .then(async ([passportResponse, marketResult]) => {
        if (!alive) return;
        setMarket(marketResult.contract);
        setOffers([...marketResult.listings, ...marketResult.passports]);
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

  const stats = useMemo(() => marketStats(rows ?? [], offers), [rows, offers]);
  const shown = useMemo(() => {
    const list = rows ?? [];
    const split = splitByOwner(list, account);
    const picked = filter === "listed" ? list.filter((row) => listedOffer(row.tokenId, offers)) : filter === "owned" ? split.owned : split.other;
    return sortMarket(picked, offers, sort);
  }, [rows, offers, account, filter, sort]);

  return (
    <main className="board market-board">
      <header className="market-head">
        <h1>Market</h1>
        <p className="lede">Testnet passport NFTs. A price appears only when the market returned an ask.</p>
        {market ? <p className="meta">Market {market}</p> : null}
      </header>
      {rows === null ? (
        <div className="market-stats" aria-hidden="true">
          <span className="market-skel stat" />
          <span className="market-skel stat" />
        </div>
      ) : (
        <div className="market-stats">
          <p><span className="meta">Passports</span> <strong>{stats.total}</strong></p>
          <p><span className="meta">Listed</span> <strong>{stats.listed}</strong></p>
          {stats.floor ? <p><span className="meta">Floor</span> <strong>{stats.floor} NEAR</strong></p> : null}
        </div>
      )}
      <div className="market-filters">
        <label className="passport-account">
          <span>{connected ? "Connected testnet account" : "Testnet account"}</span>
          <input
            value={account}
            onChange={(event) => {
              setConnected(false);
              const next = event.target.value;
              setAccount(next);
              if (!next.trim() && filter === "owned") setFilter("other");
            }}
            placeholder="name.testnet"
            autoComplete="off"
            spellCheck={false}
          />
        </label>
        <div className="passport-tabs" role="tablist" aria-label="Passport filter">
          <button type="button" role="tab" aria-selected={filter === "owned"} onClick={() => setFilter("owned")}>Owned</button>
          <button type="button" role="tab" aria-selected={filter === "other"} onClick={() => setFilter("other")}>Not owned</button>
          <button type="button" role="tab" aria-selected={filter === "listed"} onClick={() => setFilter("listed")}>Listed</button>
        </div>
        <label>
          <span className="meta">Sort</span>
          <select value={sort} onChange={(event) => setSort(event.target.value as MarketSort)}>
            <option value="id">Token id</option>
            <option value="xp">XP</option>
            <option value="price">Price</option>
          </select>
        </label>
      </div>
      {failed ? <p className="meta">The passport list did not load.</p> : null}
      {rows === null ? (
        <div className="market-grid" aria-busy="true">
          {[0, 1, 2, 3].map((slot) => (
            <article key={slot} className="market-card">
              <span className="market-skel art" />
              <span className="market-skel line" />
            </article>
          ))}
        </div>
      ) : null}
      {rows && !failed && shown.length === 0 ? (
        <p className="meta">
          {filter === "owned" && !account.trim()
            ? "Enter a testnet account, or connect a testnet wallet, to see what it owns."
            : filter === "listed"
              ? "No passports are listed."
              : "No passports in this section."}
        </p>
      ) : null}
      {rows ? (
        <div className="market-grid">
          {shown.map((row) => {
            const record = recordLine(row.career);
            const offer = listedOffer(row.tokenId, offers);
            const economy = configuredEconomy();
            const handoff = offer?.priceYocto
              ? buyHandoff(
                  { ...economy, market: offer.market ?? economy.market },
                  row.tokenId,
                  offer.priceYocto,
                  callback,
                )
              : null;
            const frame = tierFrame(row.tier);
            const media = onchainMedia(row.media);
            return (
              <article key={`${row.contract}:${row.tokenId}`} className="market-card" style={{ borderColor: frame }}>
                <a href={`/passports/${encodeURIComponent(row.tokenId)}`}>
                  <span className="passport-frame" style={{ borderColor: frame }}>
                    <PortraitStill traits={defaultSkin(row.tokenId)} fallback={media} />
                  </span>
                  <span className="market-foot">
                    <strong>{row.tokenId}</strong>
                    {row.tier ? <span className="tier-badge" style={{ borderColor: frame }}>{row.tier}</span> : null}
                    {typeof row.career?.xp === "number" ? <span>{row.career.xp} XP</span> : null}
                    {record ? <span className="meta">{record}</span> : null}
                    <span>{offer ? `${offer.priceNear} NEAR` : "Not listed"}</span>
                  </span>
                </a>
                {handoff?.ok ? <a className="btn btn-primary" href={handoff.login}>Buy {offer?.priceNear} NEAR</a> : null}
              </article>
            );
          })}
        </div>
      ) : null}
    </main>
  );
}
