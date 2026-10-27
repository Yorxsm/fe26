"use client";

import { useMemo, useState } from "react";
import { SkinPreview } from "@/components/SkinPreview";
import { audio } from "@/lib/audio";
import { defaultSkin, type SkinTraits } from "@/lib/skin";

const PREVIEW_KEY = "fe2vi.skin-preview.v1";

const ROLES = ["Safecracker", "Trader", "Hacker", "Auditor", "Builder", "Scout"];
const HATS = ["None", "Beanie", "Brim", "Hood", "Hard hat", "Antenna"];
const FACES = ["Core", "Bright", "Alarm", "Slit"];
const EXTRAS = ["None", "Headset", "Magnifier", "Goggles", "Hip kit", "Bolt"];
const TRAILS = ["None", "Sparks", "Ribbon", "Embers"];

function Stepper({
  label,
  value,
  names,
  onChange,
}: {
  label: string;
  value: number;
  names: string[];
  onChange: (next: number) => void;
}) {
  return (
    <fieldset>
      <legend>{label}</legend>
      <div className="trait-row">
        {names.map((name, index) => (
          <button key={name} type="button" aria-pressed={value === index} onClick={() => onChange(index)}>
            {name}
          </button>
        ))}
      </div>
    </fieldset>
  );
}

export default function ForgePage() {
  const seed = useMemo(() => defaultSkin("scout.vault.fe2vi.testnet"), []);
  const [traits, setTraits] = useState<SkinTraits>(seed);
  const [saved, setSaved] = useState(false);

  function set<K extends keyof SkinTraits>(key: K, value: SkinTraits[K]) {
    setSaved(false);
    setTraits((current) => ({ ...current, [key]: value }));
  }

  return (
    <main className="board">
      <h1>Skin Forge</h1>
      <p className="lede">
        A cosmetic look for an iron ingot. One NEAR on testnet. It does not change scoring, caps, or a game.
      </p>
      <p>
        You can design here. The mint is an agent spend: the registrant funds registry escrow, then the agent or its owner calls forge_mint with no attached deposit. The heist key cannot mint. This page does not submit a transaction.
      </p>
      <div className="forge-layout">
        <SkinPreview traits={traits} />
        <form
          onSubmit={(event) => {
            event.preventDefault();
            window.localStorage.setItem(PREVIEW_KEY, JSON.stringify(traits));
            setSaved(true);
            audio.unlock();
            audio.sting("forge");
          }}
        >
          <Stepper label="Body" value={traits.body} names={["Rust", "Steel", "Teal", "Gold", "Ember", "Slate", "Paper", "Bronze"]} onChange={(value) => set("body", value)} />
          <Stepper label="Face" value={traits.face} names={FACES} onChange={(value) => set("face", value)} />
          <Stepper label="Role" value={traits.role} names={ROLES} onChange={(value) => set("role", value)} />
          <Stepper label="Hat" value={traits.hat} names={HATS} onChange={(value) => set("hat", value)} />
          <Stepper label="Accessory" value={traits.accessory} names={EXTRAS} onChange={(value) => set("accessory", value)} />
          <Stepper label="Trail" value={traits.trail} names={TRAILS} onChange={(value) => set("trail", value)} />
          <p className="meta">Trail is particles in the city, not a mesh. Price 1 NEAR, storage included.</p>
          <button type="submit">Save preview and play the mint sting</button>
          {saved && <p role="status">Preview saved on this browser. It is not a minted skin.</p>}
        </form>
      </div>
    </main>
  );
}
