"use client";

import { useEffect, useMemo, useState } from "react";
import { apiBase, canonicalRules, nearToYocto, sha256Hex, withinDefaultCaps, type Rules } from "@/lib/protocol";

const initial = {
  maxPlayers: "6",
  durationSeconds: "3600",
  roundCount: "4",
  potNear: "10",
  perAgentNear: "2",
  perActionNear: "0.5",
  entryNear: "0",
};

export function CreateForm() {
  const [fields, setFields] = useState(initial);
  const [hash, setHash] = useState("");
  const [status, setStatus] = useState("");

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
      template_id: "heist-night",
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
    setStatus("");
  }

  async function preview() {
    if (!rules || problem || !hash) return;
    const base = apiBase();
    if (!base) {
      setStatus("No relay configured. Use the fe26 CLI to submit this preview on testnet.");
      return;
    }
    const response = await fetch(`${base}/v1/games`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        id: `preview-${hash.slice(0, 8)}`,
        creator: "preview.fe26.testnet",
        rules,
      }),
    });
    const body = await response.text();
    setStatus(response.ok ? `Relay accepted a preview. ${body}` : `Relay refused the preview. ${body}`);
  }

  return (
    <div className="two">
      <form
        className="card"
        onSubmit={(event) => {
          event.preventDefault();
          void preview();
        }}
      >
        <h2>Structured rules</h2>
        <p className="meta">No free-text instructions. The template is Heist Night.</p>
        <label>
          Template
          <select value="heist-night" disabled>
            <option value="heist-night">heist-night</option>
          </select>
        </label>
        <Field label="Max players" value={fields.maxPlayers} onChange={(value) => update("maxPlayers", value)} />
        <Field label="Duration seconds" value={fields.durationSeconds} onChange={(value) => update("durationSeconds", value)} />
        <Field label="Rounds" value={fields.roundCount} onChange={(value) => update("roundCount", value)} />
        <Field label="Pot (NEAR)" value={fields.potNear} onChange={(value) => update("potNear", value)} />
        <Field label="Per-agent cap (NEAR)" value={fields.perAgentNear} onChange={(value) => update("perAgentNear", value)} />
        <Field label="Per-action cap (NEAR)" value={fields.perActionNear} onChange={(value) => update("perActionNear", value)} />
        <Field label="Entry (NEAR)" value={fields.entryNear} onChange={(value) => update("entryNear", value)} />
        {problem ? <p className="error">{problem}</p> : <p className="ok">Inside the default platform caps.</p>}
        <button type="submit" disabled={Boolean(problem) || !hash}>
          Post preview to relay
        </button>
        {status && <p className="note">{status}</p>}
      </form>
      <aside className="card">
        <h2>Pinned hash</h2>
        <p className="meta">sha256 of the canonical rules JSON. This is the commitment, not a secret.</p>
        <pre>{hash || "…"}</pre>
        <pre>{canonical}</pre>
        <p className="meta">Chain submission, after you export a testnet key into the environment:</p>
        <pre>{`fe26 create-game --rules rules.json --game-md GAME.md`}</pre>
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
