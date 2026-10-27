"use client";

import { useState } from "react";
import { PenLine, Wallet } from "lucide-react";
import "@near-wallet-selector/modal-ui/styles.css";
import { connectTestnetWallet, signAndSend, type CallArgs } from "@/lib/wallet";

export function SignPanel({
  title,
  call,
  idle,
}: {
  title: string;
  call: CallArgs | null;
  idle: string;
}) {
  const [status, setStatus] = useState(idle);

  async function connect() {
    try {
      const account = await connectTestnetWallet();
      setStatus(`Connected ${account} on testnet. Nothing has been submitted.`);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Wallet selector failed to open.");
    }
  }

  async function sign() {
    if (!call) {
      setStatus("Fix the form before signing.");
      return;
    }
    try {
      const result = await signAndSend(call);
      const text = typeof result === "object" ? JSON.stringify(result) : String(result);
      setStatus(`Submitted on testnet. ${text.slice(0, 280)}`);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Signing failed.");
    }
  }

  return (
    <section className="card">
      <h2>{title}</h2>
      <p className="meta">Testnet wallet only. This page does not create an account, fund one, or accept mainnet.</p>
      <div className="row">
        <button type="button" className="btn-quiet" onClick={() => void connect()}>
          <Wallet aria-hidden="true" strokeWidth={1.75} />
          Connect testnet wallet
        </button>
        <button type="button" onClick={() => void sign()} disabled={!call}>
          <PenLine aria-hidden="true" strokeWidth={1.75} />
          Sign and send
        </button>
      </div>
      <p className="note" role="status">
        {status}
      </p>
    </section>
  );
}
