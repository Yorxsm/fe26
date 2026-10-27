"use client";

import { useEffect, useMemo, useState } from "react";
import { SignPanel } from "@/components/SignPanel";
import { canonicalRules, nearToYocto, sha256Hex, withinDefaultCaps, type Rules } from "@/lib/protocol";
import { PLACEHOLDER_CONTRACTS } from "@/lib/wallet";

const initial = {
  template: "heist-night",
  maxPlayers: "6",
  durationSeconds: "3600",
  roundCount: "4",
  potNear: "10",
  perAgentNear: "2",
  perActionNear: "0.5",
  entryNear: "0",
};

export function CreateForm() {
  const [step, setStep] = useState(0);
  const [fields, setFields] = useState(initial);
  const [hash, setHash] = useState("");

  const rules = useMemo<Rules | null>(() => {
    const pot = nearToYocto(fields.potNear);
    const agent = nearToYocto(fields.perAgentNear);
    const action = nearToYocto(fields.perActionNear);
    const entry = nearToYocto(fields.entryNear);
    const maxPlayers = Number(fields.maxPlayers);
    const durationSeconds = Number(fields.durationSeconds);
    const roundCount = Number(fields.roundCount);
    if (pot === null || agent === null || action === null || entry === null) return null;
    if (![maxPlayers, durationSeconds, roundCount].every((value) => Number.isInteger(value) && value >= 0)) return null;
    return {
      template_id: fields.template,
      max_players: maxPlayers,
      duration_seconds: durationSeconds,
      round_count: roundCount,
      pot_yocto: pot,
      per_agent_cap_yocto: agent,
      per_action_cap_yocto: action,
      entry_yocto: entry,
    };
  }, [fields]);

  const problem = rules ? withinDefaultCaps(rules) : "Enter whole numbers and NEAR amounts with at most 24 decimal places.";
  const canonical = rules ? canonicalRules(rules) : "";

  useEffect(() => {
    if (!canonical) {
      setHash("");
      return;
    }
    let cancelled = false;
    sha256Hex(canonical).then((value) => {
      if (!cancelled) setHash(value);
    });
    return () => {
      cancelled = true;
    };
  }, [canonical]);

  function update(name: keyof typeof initial, value: string) {
    setFields((current) => ({ ...current, [name]: value }));
  }

  const call =
    rules && !problem && hash
      ? {
          receiverId: PLACEHOLDER_CONTRACTS.factory,
          methodName: "create_game",
          args: { rules, game_md_hash: hash },
          deposit: rules.pot_yocto,
          gas: "200000000000000",
        }
      : null;

  return (
    <div className="two">
      <form
        className="card"
        onSubmit={(event) => {
          event.preventDefault();
          if (!problem) setStep(Math.min(step + 1, 2));
        }}
      >
        <ol className="steps">
          {["Rules", "Caps", "Sign"].map((label, index) => (
            <li key={label}>
              <button type="button" className={step === index ? "on" : ""} onClick={() => setStep(index)}>
                {index + 1}. {label}
              </button>
            </li>
          ))}
        </ol>
        {step === 0 && (
          <>
            <label>
              Template
              <select value={fields.template} onChange={(event) => update("template", event.target.value)}>
                <option value="heist-night">heist-night</option>
                <option value="trading-cup">trading-cup</option>
              </select>
            </label>
            <Field label="Max players" value={fields.maxPlayers} onChange={(value) => update("maxPlayers", value)} />
            <Field label="Duration seconds" value={fields.durationSeconds} onChange={(value) => update("durationSeconds", value)} />
            <Field label="Rounds" value={fields.roundCount} onChange={(value) => update("roundCount", value)} />
          </>
        )}
        {step === 1 && (
          <>
            <Field label="Pot (NEAR)" value={fields.potNear} onChange={(value) => update("potNear", value)} />
            <Field label="Per-agent cap (NEAR)" value={fields.perAgentNear} onChange={(value) => update("perAgentNear", value)} />
            <Field label="Per-action cap (NEAR)" value={fields.perActionNear} onChange={(value) => update("perActionNear", value)} />
            <Field label="Entry (NEAR)" value={fields.entryNear} onChange={(value) => update("entryNear", value)} />
          </>
        )}
        {step === 2 && (
          <p className="meta">
            Signing calls create_game on {PLACEHOLDER_CONTRACTS.factory} and attaches the pot. The CLI dry-run remains available if you do not connect a wallet.
          </p>
        )}
        {problem ? <p className="error">{problem}</p> : <p className="ok">Inside the default platform caps.</p>}
        <button type="submit">{step < 2 ? "Next" : "Review stays on this page"}</button>
      </form>
      <aside className="card">
        <h2>Pinned hash</h2>
        <p className="meta">sha256 of the canonical rules JSON. This is the commitment, not a secret.</p>
        <pre>{hash || "…"}</pre>
        <pre>{canonical}</pre>
        {step === 2 && <SignPanel title="Sign create_game" call={call} idle="Connect a testnet wallet when you want to list this game." />}
      </aside>
    </div>
  );
}

function Field({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) {
  return (
    <label>
      {label}
      <input value={value} onChange={(event) => onChange(event.target.value)} inputMode="decimal" />
    </label>
  );
}
