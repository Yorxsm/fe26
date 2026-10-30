"use client";

import { useEffect, useState } from "react";
import {
  fetchHyperAccount,
  formatSignedUsd,
  formatUsd,
  type HyperAccount,
} from "@/lib/equity";
import { shortAddress } from "@/lib/arena";

export function EquityPanel({ address }: { address: string }) {
  const [account, setAccount] = useState<HyperAccount | null>(null);
  const [phase, setPhase] = useState<"loading" | "ready" | "offline">("loading");

  useEffect(() => {
    let alive = true;
    const load = async () => {
      const result = await fetchHyperAccount(address);
      if (!alive) return;
      setAccount(result);
      setPhase(result ? "ready" : "offline");
    };
    void load();
    const timer = setInterval(load, 30_000);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [address]);

  return (
    <section className="card" aria-label="Hyperliquid account">
      <div className="row">
        <h3>Hyperliquid account</h3>
        <span className={account?.source === "hyperliquid" ? "tag tag-live" : "tag"}>
          {account?.source === "hyperliquid" ? "live" : "read-only"}
        </span>
      </div>
      <p className="meta addr">{shortAddress(address)}</p>
      {phase === "loading" ? <p className="muted">Reading clearinghouse state…</p> : null}
      {phase === "offline" ? (
        <p className="muted">The public account read is offline. No order is sent.</p>
      ) : null}
      {account && account.source === "unavailable" ? (
        <p className="muted">Hyperliquid did not answer. No order is sent.</p>
      ) : null}
      {account && account.source === "hyperliquid" ? (
        <>
          <ul className="feed">
            <li className="row">
              <span>Account value</span>
              <strong className="num">{formatUsd(account.accountValue)}</strong>
            </li>
            <li className="row">
              <span>Withdrawable</span>
              <strong className="num">{formatUsd(account.withdrawable)}</strong>
            </li>
            <li className="row">
              <span>Margin used</span>
              <strong className="num">{formatUsd(account.marginUsed)}</strong>
            </li>
          </ul>
          <h4>Open positions</h4>
          {account.positions.length === 0 ? (
            <p className="muted">No open positions.</p>
          ) : (
            <ul className="feed">
              {account.positions.map((position) => (
                <li key={position.coin} className="row">
                  <span className="addr">
                    {position.coin} · {position.size > 0 ? "long" : "short"} {Math.abs(position.size)}
                    {" · "}
                    {position.leverage}x {position.leverageType}
                  </span>
                  <span
                    className={position.unrealizedPnl >= 0 ? "num status-active" : "num status-revoked"}
                  >
                    {formatSignedUsd(position.unrealizedPnl)}
                  </span>
                </li>
              ))}
            </ul>
          )}
          <p className="meta">
            Equity and positions come from the public clearinghouse state. fe2VI cannot trade this account.
          </p>
        </>
      ) : null}
    </section>
  );
}
