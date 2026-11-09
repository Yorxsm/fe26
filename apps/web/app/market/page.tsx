"use client";

import Link from "next/link";
import { useEffect, useState, useMemo } from "react";
import { Eye, Shield, Sparkles, Trophy, TrendingUp, ShoppingBag, ExternalLink } from "lucide-react";
import { SkinPreview } from "@/components/SkinPreview";
import {
  fetchMarketData,
  type MarketListing,
  type MarketSale,
  type MarketStats,
} from "@/lib/market";
import { PAINT_COLORS, SCREEN_COLORS, ROLE_NAMES, TRAIL_NAMES } from "@/lib/skin";
import { buyHandoff, configuredEconomy } from "@/lib/nearBuy";

const EMPTY_STATS: MarketStats = {
  floorPriceNear: null,
  totalVolumeNear: null,
  listedCount: 0,
  totalSalesCount: null,
  activeSpectators: null,
};

export default function MarketPage() {
  const [listings, setListings] = useState<MarketListing[]>([]);
  const [passports, setPassports] = useState<MarketListing[]>([]);
  const [sales, setSales] = useState<MarketSale[]>([]);
  const [stats, setStats] = useState<MarketStats>(EMPTY_STATS);
  const [source, setSource] = useState<"chain" | "not-read" | "unavailable" | "loading">("loading");
  const [contract, setContract] = useState("");
  const [network, setNetwork] = useState("");
  const [blockers, setBlockers] = useState<string[]>([]);
  const [lastSettlement, setLastSettlement] = useState<{ buyer?: string; seller?: string; priceNear?: string; note?: string } | null>(null);
  const [selectedId, setSelectedId] = useState<string>("");
  const [rarityFilter, setRarityFilter] = useState<string>("all");
  const [sortOrder, setSortOrder] = useState<"price-asc" | "price-desc" | "rank">("price-asc");

  useEffect(() => {
    let alive = true;
    fetchMarketData().then((result) => {
      if (!alive) return;
      setListings(result.listings);
      setPassports(result.passports);
      setSales(result.sales);
      const first = result.passports[0]?.tokenId ?? result.listings[0]?.tokenId;
      if (first) setSelectedId(first);
      setStats(result.stats);
      setSource(result.source);
      setContract(result.contract);
      setNetwork(result.network);
      setBlockers(result.blockers);
      setLastSettlement(result.lastSettlement);
    });
    return () => {
      alive = false;
    };
  }, []);

  const inventory = passports.length ? passports : listings;
  const filteredListings = useMemo(() => {
    let result = [...inventory];
    if (rarityFilter !== "all") {
      result = result.filter((item) => (item.rarity ?? "").toLowerCase() === rarityFilter.toLowerCase());
    }
    if (sortOrder === "price-asc") {
      result.sort((a, b) => parseFloat(a.priceNear || "0") - parseFloat(b.priceNear || "0"));
    } else if (sortOrder === "price-desc") {
      result.sort((a, b) => parseFloat(b.priceNear || "0") - parseFloat(a.priceNear || "0"));
    } else if (sortOrder === "rank") {
      result.sort((a, b) => (a.career?.rank ?? 0) - (b.career?.rank ?? 0));
    }
    return result;
  }, [inventory, rarityFilter, sortOrder]);

  const selected = useMemo(() => {
    return inventory.find((item) => item.tokenId === selectedId) ?? inventory[0] ?? null;
  }, [inventory, selectedId]);

  return (
    <main className="board">
      <div className="row" style={{ alignItems: "flex-start", justifyContent: "space-between" }}>
        <div>
          <div className="row" style={{ gap: "0.75rem", marginBottom: "0.25rem" }}>
            <h1>NFT Market</h1>
            <span className="tag tag-live" style={{ display: "inline-flex", alignItems: "center", gap: "0.35rem" }}>
              <Eye size={13} /> Spectator View
            </span>
          </div>
          <p className="lede">
            Listings read from the configured testnet market. Buying opens a testnet wallet and does not run until you confirm it there.
            {contract ? <> Market: <code>{contract}</code>.</> : null}
            {network ? <> Chain: <code>{network}</code>.</> : null}
          </p>
        </div>
      </div>

      <p className="meta">
        {source === "loading"
          ? "Reading the market contract."
          : source === "chain"
            ? inventory.length
              ? listings.length
                ? "These asks were returned by the market contract."
                : "The passport contract returned tokens. None of them has an ask stored right now."
              : "The market contract has no listings and the passport contract returned no tokens."
            : source === "not-read"
              ? "This response did not read the chain."
              : "The market contract could not be read. This page is not showing a sample."}
        {blockers.map((blocker) => (
          <span key={blocker}> {blocker}</span>
        ))}
      </p>

      {/* Market Metrics Strip */}
      <div className="grid" style={{ gridTemplateColumns: "repeat(auto-fit, minmax(140px, 1fr))", gap: "1rem" }}>
        <div className="card" style={{ padding: "1rem" }}>
          <div className="meta" style={{ display: "flex", alignItems: "center", gap: "0.4rem" }}>
            <TrendingUp size={14} color="#f0883e" /> Floor Price
          </div>
          <div className="type-h1" style={{ fontSize: "1.75rem", lineHeight: "2.25rem", color: "var(--ink)" }}>
            {stats.floorPriceNear ?? "—"} <span className="meta" style={{ fontSize: "0.9rem" }}>NEAR</span>
          </div>
        </div>
        <div className="card" style={{ padding: "1rem" }}>
          <div className="meta" style={{ display: "flex", alignItems: "center", gap: "0.4rem" }}>
            <Sparkles size={14} color="#3fb950" /> Recorded volume
          </div>
          <div className="type-h1" style={{ fontSize: "1.75rem", lineHeight: "2.25rem", color: "var(--ink)" }}>
            {stats.totalVolumeNear ?? "—"} <span className="meta" style={{ fontSize: "0.9rem" }}>NEAR</span>
          </div>
        </div>
        <div className="card" style={{ padding: "1rem" }}>
          <div className="meta" style={{ display: "flex", alignItems: "center", gap: "0.4rem" }}>
            <ShoppingBag size={14} /> Listed Skins
          </div>
          <div className="type-h1" style={{ fontSize: "1.75rem", lineHeight: "2.25rem", color: "var(--ink)" }}>
            {stats.listedCount}
          </div>
        </div>
        <div className="card" style={{ padding: "1rem" }}>
          <div className="meta" style={{ display: "flex", alignItems: "center", gap: "0.4rem" }}>
            <Shield size={14} /> Lifetime sales
          </div>
          <div className="type-h1" style={{ fontSize: "1.75rem", lineHeight: "2.25rem", color: "var(--ink)" }}>
            {stats.totalSalesCount ?? "—"}
          </div>
        </div>
        <div className="card" style={{ padding: "1rem" }}>
          <div className="meta" style={{ display: "flex", alignItems: "center", gap: "0.4rem" }}>
            <Eye size={14} color="#58a6ff" /> Spectators
          </div>
          <div className="type-h1" style={{ fontSize: "1.75rem", lineHeight: "2.25rem", color: "var(--forge)" }}>
            {stats.activeSpectators ?? "—"}
          </div>
        </div>
      </div>

      {selected ? (
      <div className="forge-stage">
        <section className="lobby-panel" style={{ position: "relative" }}>
          {selected.traits ? <SkinPreview traits={selected.traits} /> : <p className="meta">This chain listing has no cosmetic preview.</p>}
          <div style={{ position: "absolute", top: "1rem", left: "1rem", display: "flex", gap: "0.5rem" }}>
            {selected.rarity ? <span className={selected.rarity === "Mythic" ? "tag tag-hot" : "tag"}>{selected.rarity}</span> : null}
            <span className="tag">{selected.tokenId}</span>
          </div>
          <p className="meta" style={{ textAlign: "center", marginTop: "0.5rem" }}>
            Orbit to inspect 3D dynamic skin, custom headwear, visor color, and trail emitter.
          </p>
        </section>

        <section className="card" style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
          <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-start" }}>
            <div>
              <h2 style={{ margin: 0 }}>{selected.name}</h2>
              <span className="meta">
                Seller: <code style={{ color: "var(--forge)" }}>{selected.seller}</code>
              </span>
            </div>
            <div style={{ textAlign: "right" }}>
              <div style={{ fontSize: "1.75rem", fontWeight: 700, color: "var(--ink)" }}>
                {selected.listed === false || !selected.priceNear ? "Not listed" : <>{selected.priceNear} <span style={{ fontSize: "1rem", color: "var(--ink-secondary)" }}>NEAR</span></>}
              </div>
              <span className="meta" style={{ color: "var(--ink-secondary)" }}>0% platform fee</span>
            </div>
          </div>

          <p>{selected.description}</p>

          {/* Dynamic Cosmetics */}
          <div>
            <h3 style={{ fontSize: "0.95rem", textTransform: "uppercase", letterSpacing: "0.05em", color: "var(--ink-secondary)" }}>
              Cosmetic Traits
            </h3>
            <div className="forge-swatches" style={{ marginTop: "0.5rem" }}>
              <span
                className="tag"
                style={{
                  background: PAINT_COLORS[(selected.traits?.paint ?? 0) % PAINT_COLORS.length],
                  color: "#101012",
                  fontWeight: 600,
                }}
              >
                Paint
              </span>
              <span
                className="tag"
                style={{
                  background: SCREEN_COLORS[(selected.traits?.screen ?? 0) % SCREEN_COLORS.length],
                  color: "#101012",
                  fontWeight: 600,
                }}
              >
                Screen Visor
              </span>
              <span className="tag">
                {selected.traits ? ROLE_NAMES[selected.traits.role % ROLE_NAMES.length] : "not on chain"}
              </span>
              <span className="tag">
                {selected.traits ? TRAIL_NAMES[selected.traits.trail % TRAIL_NAMES.length] : "not on chain"} Trail
              </span>
            </div>
          </div>

          {/* Battle Trophies */}
          <div>
            <h3 style={{ fontSize: "0.95rem", textTransform: "uppercase", letterSpacing: "0.05em", color: "var(--ink-secondary)" }}>
              Chain record
            </h3>
            {(selected.trophies ?? []).length === 0 ? (
              <p className="meta" style={{ fontStyle: "italic" }}>No competitive battle trophies attached yet.</p>
            ) : (
              <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "0.5rem", marginTop: "0.5rem" }}>
                {(selected.trophies ?? []).map((trophy, idx) => (
                  <div
                    key={idx}
                    className="card"
                    style={{
                      padding: "0.5rem 0.75rem",
                      background: "rgba(255, 255, 255, 0.03)",
                      display: "flex",
                      alignItems: "center",
                      gap: "0.6rem",
                    }}
                  >
                    <span style={{ fontSize: "1.4rem" }}>{trophy.icon}</span>
                    <div>
                      <div style={{ fontWeight: 600, fontSize: "0.85rem" }}>{trophy.name}</div>
                      <div className="meta" style={{ fontSize: "0.75rem", lineHeight: "1rem" }}>{trophy.description}</div>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Career Record */}
          <div style={{ borderTop: "1px solid var(--line)", paddingTop: "0.75rem" }}>
            <h3 style={{ fontSize: "0.95rem", textTransform: "uppercase", letterSpacing: "0.05em", color: "var(--ink-secondary)" }}>
              Career Reputation
            </h3>
            <div className="grid" style={{ gridTemplateColumns: "repeat(4, 1fr)", gap: "0.5rem", marginTop: "0.5rem" }}>
              <div style={{ textAlign: "center", padding: "0.4rem", background: "rgba(255, 255, 255, 0.02)" }}>
                <div className="meta">XP</div>
                <div style={{ fontWeight: 700 }}>{selected.career?.xp ?? "—"}</div>
              </div>
              <div style={{ textAlign: "center", padding: "0.4rem", background: "rgba(255, 255, 255, 0.02)" }}>
                <div className="meta">Matches</div>
                <div style={{ fontWeight: 700 }}>{selected.career?.matches ?? "—"}</div>
              </div>
              <div style={{ textAlign: "center", padding: "0.4rem", background: "rgba(255, 255, 255, 0.02)" }}>
                <div className="meta">Wins</div>
                <div style={{ fontWeight: 700, color: "var(--green)" }}>{selected.career?.wins ?? "—"}</div>
              </div>
              <div style={{ textAlign: "center", padding: "0.4rem", background: "rgba(255, 255, 255, 0.02)" }}>
                <div className="meta">Rank</div>
                <div style={{ fontWeight: 700, color: "var(--forge)" }}>{selected.career ? `#${selected.career.rank}` : "—"}</div>
              </div>
            </div>
            {selected.vaultReceipts || selected.career?.vaultReceipts ? (
              <p className="meta" style={{ marginTop: "0.6rem" }}>
                Vault receipt record on this token:{" "}
                {(selected.vaultReceipts ?? selected.career?.vaultReceipts)?.wins ?? 0} wins,{" "}
                {(selected.vaultReceipts ?? selected.career?.vaultReceipts)?.draws ?? 0} draws,{" "}
                {(selected.vaultReceipts ?? selected.career?.vaultReceipts)?.losses ?? 0} losses, payout{" "}
                {(selected.vaultReceipts ?? selected.career?.vaultReceipts)?.payoutYocto ?? "0"} yocto.
                Stays with the token after transfer. Binding does not. Source: settled receipts.
              </p>
            ) : (
              <p className="meta" style={{ marginTop: "0.6rem" }}>
                No Vault receipt record is attached to this token.
              </p>
            )}
          </div>

          <div style={{ borderTop: "1px solid var(--line)", paddingTop: "0.75rem" }}>
            <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
              <span className="meta" style={{ fontWeight: 600 }}>Buy on testnet</span>
              <span className="meta">Contract: <code>{contract || "not configured"}</code></span>
            </div>
            {selected.listed === false || !selected.priceYocto ? (
              <p className="meta">This passport has no ask on the market contract.</p>
            ) : (() => {
              const callback = typeof window === "undefined" ? "https://fe2vi.space/market" : window.location.href.split("?")[0];
              const handoff = buyHandoff(configuredEconomy(), selected.tokenId, selected.priceYocto, callback);
              if (!handoff.ok) return <p className="meta">{handoff.reason}</p>;
              return (
                <>
                  <a className="btn btn-primary" href={handoff.login}>Open testnet wallet to buy {selected.priceNear} NEAR</a>
                  <p className="meta" style={{ marginTop: "0.4rem" }}>
                    Calls <code>{handoff.method}</code> on <code>{handoff.receiverId}</code> with <code>{handoff.deposit}</code> yoctoNEAR. The wallet asks before anything is signed. <a href={handoff.explorer}>View the contract</a>.
                  </p>
                  <pre className="forge-call" style={{ margin: "0.5rem 0 0 0", fontSize: "0.8rem" }}>
{`fe2vi passport buy --token-id ${selected.tokenId} --price ${selected.priceNear}`}
                  </pre>
                </>
              );
            })()}
          </div>
        </section>
      </div>
      ) : sales.length === 0 ? (
        <section className="card">
          <h2>No market records</h2>
          <p>The testnet market returned no asks, no passports, and no stored settlement.</p>
        </section>
      ) : null}

      {/* Filter and Browse Listings */}
      <section>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: "1rem" }}>
          <div>
            <h2>Passports ({filteredListings.length})</h2>
            <p className="meta">Tokens on the passport contract. An ask appears only when the market stores one.</p>
          </div>
          <div className="row" style={{ gap: "0.5rem" }}>
            <div className="forge-tabs" style={{ margin: 0 }}>
              {["all", "mythic", "rare", "common"].map((tier) => (
                <button
                  key={tier}
                  type="button"
                  aria-selected={rarityFilter === tier}
                  onClick={() => setRarityFilter(tier)}
                  style={{ textTransform: "capitalize", padding: "0.3rem 0.75rem", fontSize: "0.85rem" }}
                >
                  {tier}
                </button>
              ))}
            </div>
            <select
              value={sortOrder}
              onChange={(e) => setSortOrder(e.target.value as any)}
              style={{
                background: "var(--surface-raised)",
                color: "var(--ink)",
                border: "1px solid var(--line)",
                padding: "0.35rem 0.6rem",
                borderRadius: "4px",
                fontSize: "0.85rem",
              }}
            >
              <option value="price-asc">Price: Low to High</option>
              <option value="price-desc">Price: High to Low</option>
              <option value="rank">Highest Rank</option>
            </select>
          </div>
        </div>

        <div className="grid" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(260px, 1fr))", gap: "1rem", marginTop: "1rem" }}>
          {filteredListings.map((listing) => {
            const isSelected = listing.tokenId === selectedId;
            return (
              <div
                key={listing.tokenId}
                className="card"
                onClick={() => setSelectedId(listing.tokenId)}
                style={{
                  cursor: "pointer",
                  border: isSelected ? "2px solid var(--forge)" : "1px solid var(--line)",
                  padding: "1rem",
                  display: "flex",
                  flexDirection: "column",
                  justifyContent: "space-between",
                  transition: "all 0.15s ease",
                  background: isSelected ? "rgba(240, 136, 62, 0.05)" : "var(--surface)",
                }}
              >
                <div>
                  <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
                    <span className="meta" style={{ fontFamily: "monospace" }}>{listing.tokenId}</span>
                    {listing.rarity ? <span className={listing.rarity === "Mythic" ? "tag tag-hot" : "tag"}>{listing.rarity}</span> : null}
                  </div>
                  <h3 style={{ margin: "0.5rem 0 0.25rem 0", fontSize: "1.05rem" }}>{listing.name}</h3>
                  <div className="meta" style={{ color: "var(--ink-secondary)" }}>
                    Seller: {listing.seller}
                  </div>
                  <div className="stamps" style={{ margin: "0.6rem 0" }}>
                    {(listing.trophies ?? []).map((t, idx) => (
                      <span key={idx} className="stamp neutral" title={t.name} style={{ fontSize: "0.8rem", padding: "0.15rem 0.4rem" }}>
                        {t.icon} {t.name}
                      </span>
                    ))}
                  </div>
                </div>

                <div style={{ borderTop: "1px solid var(--line)", paddingTop: "0.6rem", marginTop: "0.6rem" }}>
                  <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
                    <span className="meta">{listing.owner ? `Owner ${listing.owner}` : listing.listed === false ? "Not listed" : "Listed"}</span>
                    <span style={{ fontWeight: 700, fontSize: "1.1rem", color: "var(--forge)" }}>
                      {listing.listed === false || !listing.priceNear ? "Not listed" : `${listing.priceNear} NEAR`}
                    </span>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </section>

      {/* Live On-Chain Sales Feed */}
      <section style={{ marginTop: "1.5rem" }}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
          <h2>Sales tape</h2>
          <span className="meta">
            {lastSettlement
              ? `Last settlement only: ${lastSettlement.seller ?? "unknown"} to ${lastSettlement.buyer ?? "unknown"} for ${lastSettlement.priceNear ?? "?"} NEAR. This is not a lifetime volume.`
              : "No settlement is stored. Lifetime volume is not indexed."}
          </span>
        </div>
        <table className="board" style={{ marginTop: "0.75rem" }}>
          <thead>
            <tr>
              <th>Time</th>
              <th>Token</th>
              <th>Buyer</th>
              <th>Seller</th>
              <th>Price</th>
              <th>Settlement</th>
            </tr>
          </thead>
          <tbody>
            {sales.length === 0 ? (
              <tr>
                <td colSpan={6}>No settlement is stored.</td>
              </tr>
            ) : null}
            {sales.map((sale) => (
              <tr key={sale.id}>
                <td className="meta">{sale.timestamp || "Stored settlement"}</td>
                <td>
                  <span style={{ fontWeight: 600 }}>{sale.tokenId || "—"}</span>
                  {sale.rarity ? (
                    <span className={sale.rarity === "Mythic" ? "tag tag-hot" : "tag"} style={{ fontSize: "0.7rem", padding: "0.1rem 0.3rem" }}>
                      {sale.rarity}
                    </span>
                  ) : null}
                </td>
                <td className="addr">{sale.buyer}</td>
                <td className="addr">{sale.seller}</td>
                <td className="num" style={{ fontWeight: 700, color: "var(--forge)" }}>
                  {sale.priceNear} NEAR
                </td>
                <td>
                  <span className="tag tag-live" style={{ fontSize: "0.75rem" }}>
                    NEP-199 Direct
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
    </main>
  );
}
