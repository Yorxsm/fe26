"use client";

import { useEffect, useState } from "react";
import { apiBase } from "@/lib/protocol";

type Step = { id: string; state: "pending" | "done" | "failed" | string; detail?: string | null };
type Receipt = { matchId?: string; txHash?: string; txUrl?: string; outcome?: string };
type PassportCard = {
  tokenId?: string | null;
  status?: string;
  bound?: boolean;
  ownerNear?: string;
  career?: { wins?: number; draws?: number; losses?: number; payoutYocto?: string; source?: string } | null;
  receipts?: Receipt[];
};
type AgentCard = {
  handle: string;
  name?: string;
  veteran?: boolean;
  receiptWins?: number;
  passport?: PassportCard | null;
};
type JudgeBody = {
  configured?: boolean;
  potCapNear?: string;
  sendsNear?: boolean;
  retryAfter?: number;
  windowSecs?: number;
  match?: {
    matchId?: string;
    status?: string;
    outcome?: string | null;
    seats?: string[];
    roomUrl?: string;
    watchUrl?: string;
    payNote?: string;
    txHash?: string | null;
    txUrl?: string | null;
  } | null;
  steps?: Step[];
  agents?: AgentCard[];
  transfers?: PassportCard[];
  error?: string;
  message?: string;
};

const LABELS: Record<string, string> = {
  queued: "Queued",
  live: "Match live",
  result: "Result",
  receipt: "Payout receipt",
  career: "Career updated",
};

export default function JudgePage() {
  const [body, setBody] = useState<JudgeBody | null>(null);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState("Reading the judge desk.");

  useEffect(() => {
    let dead = false;
    const pull = () => {
      fetch(`${apiBase()}/v1/judge`, { cache: "no-store" })
        .then((response) => response.json())
        .then((next: JudgeBody) => {
          if (dead) return;
          setBody(next);
          setNote(next.message || (next.configured ? "Ready." : "Judge agents are not configured on this API."));
        })
        .catch(() => {
          if (!dead) setNote("The judge desk did not answer.");
        });
    };
    pull();
    const timer = window.setInterval(pull, 4000);
    return () => {
      dead = true;
      window.clearInterval(timer);
    };
  }, []);

  const run = () => {
    setBusy(true);
    fetch(`${apiBase()}/v1/judge`, { method: "POST", cache: "no-store" })
      .then(async (response) => ({ status: response.status, body: (await response.json()) as JudgeBody }))
      .then((next) => {
        setBody(next.body);
        setNote(next.body.message || (next.status === 200 ? "Match recorded." : "The match did not start."));
      })
      .catch(() => setNote("The judge desk did not answer."))
      .finally(() => setBusy(false));
  };

  const match = body?.match;
  const waiting = (body?.retryAfter ?? 0) > 0;

  return (
    <main className="board">
      <h1>Judge</h1>
      <p className="lede">
        One Vault match between the two house testnet agents. The pot cap is {body?.potCapNear ?? "0.01"} NEAR a seat.
        This page does not send NEAR. A payout appears only after a FINAL settle_vault receipt.
      </p>
      <p className="meta">{note}{body?.sendsNear === false ? " No transaction is sent from here." : ""}</p>
      <button type="button" className="btn btn-primary" onClick={run} disabled={busy || body?.configured === false || waiting}>
        {busy ? "Starting" : waiting ? `Wait ${body?.retryAfter}s` : "Run a match"}
      </button>
      <ol className="feed">
        {(body?.steps ?? []).map((step) => (
          <li key={step.id}>
            <strong>{LABELS[step.id] ?? step.id}</strong> · {step.state}
            {step.id === "live" && match?.matchId ? (
              <>
                {" "}
                <a href={match.watchUrl || "/games/heist-night"}>{match.matchId}</a>
                {match.roomUrl ? <> · <a href={match.roomUrl}>Vault room</a></> : null}
              </>
            ) : null}
            {step.id === "result" && match?.outcome ? <> · {match.outcome}. {match.payNote}</> : null}
            {step.id === "receipt" && match?.txUrl ? (
              <>
                {" "}
                <a href={match.txUrl}>{match.txHash}</a>
              </>
            ) : null}
            {step.id === "receipt" && step.state === "pending" && match?.status === "over" ? " · Waiting for the settler receipt." : null}
            {step.id === "career" && step.state === "pending" && match?.status === "over" ? " · Career updates when that receipt is accepted." : null}
          </li>
        ))}
      </ol>
      <section className="card">
        <h2>Passports</h2>
        {(body?.agents ?? []).length === 0 ? <p className="meta">No judge agents are stored yet.</p> : null}
        <ul className="feed">
          {(body?.agents ?? []).map((agent) => (
            <li key={agent.handle}>
              <a href={`/agents/${agent.handle}`}>{agent.handle}</a>
              {agent.veteran ? " · veteran" : ""}
              {typeof agent.receiptWins === "number" ? ` · ${agent.receiptWins} receipt wins` : ""}
              {agent.passport ? (
                <PassportDetail card={agent.passport} />
              ) : (
                <p className="meta">No passport is bound to this agent in the arena book.</p>
              )}
            </li>
          ))}
        </ul>
      </section>
      <section className="card">
        <h2>Transfer</h2>
        <p className="meta">
          A transfer clears the agent binding and leaves the receipt career on the token. The new owner binds their own agent. This list is only tokens the arena book already shows as unbound with a receipt career.
        </p>
        {(body?.transfers ?? []).length === 0 ? <p>No transferred passport with a receipt career is recorded.</p> : null}
        <ul className="feed">
          {(body?.transfers ?? []).map((card) => (
            <li key={card.tokenId || card.ownerNear}>
              <PassportDetail card={card} />
              <p className="meta">Binding is clear. The career above stayed on the token.</p>
            </li>
          ))}
        </ul>
      </section>
    </main>
  );
}

function PassportDetail({ card }: { card: PassportCard }) {
  const career = card.career && typeof card.career === "object" ? card.career : null;
  return (
    <div>
      <p>
        {card.tokenId || "no token"} · {card.status || "unknown"}
        {card.bound ? " · bound" : " · not bound"}
        {card.ownerNear ? ` · ${card.ownerNear}` : ""}
      </p>
      <p className="meta">
        {career
          ? `${career.wins ?? 0} wins, ${career.draws ?? 0} draws, ${career.losses ?? 0} losses, payout ${career.payoutYocto ?? "0"} yocto. Source: ${career.source ?? "arena book"}.`
          : "No receipt career on this token."}
      </p>
      {(card.receipts ?? []).length > 0 ? (
        <ul>
          {(card.receipts ?? []).map((receipt) => (
            <li key={`${receipt.matchId}-${receipt.txHash}`}>
              {receipt.outcome} · {receipt.matchId} · {receipt.txUrl ? <a href={receipt.txUrl}>{receipt.txHash}</a> : receipt.txHash}
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}
