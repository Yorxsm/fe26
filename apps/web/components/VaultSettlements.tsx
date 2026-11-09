"use client";

import { useEffect, useState } from "react";
import { apiBase } from "@/lib/protocol";

type Settlement = {
  matchId: string;
  outcome: string;
  rule: string;
  txHash: string;
  txUrl: string;
  source: string;
};

type Payload = {
  source?: string;
  note?: string;
  matches?: Settlement[];
};

export function VaultSettlements() {
  const [rows, setRows] = useState<Settlement[] | null>(null);
  const [note, setNote] = useState("Receipt-verified Vault matches.");

  useEffect(() => {
    const base = apiBase();
    let stopped = false;
    fetch(`${base}/v1/vault/settlements`)
      .then((response) => (response.ok ? response.json() : null))
      .then((body: Payload | null) => {
        if (stopped) return;
        setRows(Array.isArray(body?.matches) ? body.matches : []);
        if (body?.note) setNote(body.note);
      })
      .catch(() => {
        if (!stopped) setRows([]);
      });
    return () => {
      stopped = true;
    };
  }, []);

  return (
    <section className="lobby-panel lobby-span-12 ui-in">
      <h2>Vault loop</h2>
      <p className="meta">{note} Source: settled receipts. Unsettled matches stay off this list.</p>
      {rows === null ? <p className="meta">Reading the receipt list.</p> : null}
      {rows && rows.length === 0 ? <p className="meta">No settled Vault receipt is recorded.</p> : null}
      {rows && rows.length > 0 ? (
        <ul className="lobby-stack">
          {rows.map((row) => (
            <li key={row.matchId}>
              <span className="meta">{row.matchId}</span> {row.outcome} · {row.rule} ·{" "}
              <a href={row.txUrl}>{row.txHash}</a>
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  );
}
