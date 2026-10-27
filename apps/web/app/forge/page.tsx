"use client";

import { useMemo, useState } from "react";
import { Backpack, Badge, Check, Dices, Eye, Hammer, HardHat, Monitor, Palette, Shirt, Smile, Sparkles, type LucideIcon } from "lucide-react";
import { SkinPreview } from "@/components/SkinPreview";
import { TrophyShelf } from "@/components/TrophyShelf";
import { Button, Panel } from "@/components/ui";
import { audio } from "@/lib/audio";
import {
  BACK_ITEM_NAMES,
  BACK_ITEMS,
  EMBLEM_NAMES,
  EMBLEMS,
  EYE_NAMES,
  EYE_STYLES,
  FACE_ITEM_NAMES,
  FACE_ITEMS,
  HEADWEAR,
  HEADWEAR_NAMES,
  PAINT_COLORS,
  PAINT_NAMES,
  PAINTS,
  ROLE_NAMES,
  ROLE_OUTFITS,
  SCREEN_COLORS,
  SCREEN_NAMES,
  SCREENS,
  TRAIL_NAMES,
  TRAILS,
  defaultSkin,
  rarity,
  type SkinTraits,
} from "@/lib/skin";
import { PLACEHOLDER_CONTRACTS, assertTestnet } from "@/lib/wallet";

const PREVIEW_KEY = "fe2vi.skin-preview.v1";

const CATEGORIES: Array<{
  key: keyof SkinTraits;
  label: string;
  names: string[];
  count: number;
  colors?: string[];
  icon: LucideIcon;
}> = [
  { key: "paint", label: "Paint", names: PAINT_NAMES, count: PAINTS, colors: PAINT_COLORS, icon: Palette },
  { key: "screen", label: "Screen", names: SCREEN_NAMES, count: SCREENS, colors: SCREEN_COLORS, icon: Monitor },
  { key: "eyes", label: "Eyes", names: EYE_NAMES, count: EYE_STYLES, icon: Eye },
  { key: "headwear", label: "Headwear", names: HEADWEAR_NAMES, count: HEADWEAR, icon: HardHat },
  { key: "face_item", label: "Face", names: FACE_ITEM_NAMES, count: FACE_ITEMS, icon: Smile },
  { key: "back_item", label: "Back", names: BACK_ITEM_NAMES, count: BACK_ITEMS, icon: Backpack },
  { key: "emblem", label: "Emblem", names: EMBLEM_NAMES, count: EMBLEMS, icon: Badge },
  { key: "trail", label: "Trail", names: TRAIL_NAMES, count: TRAILS, icon: Sparkles },
  { key: "role", label: "Role", names: ROLE_NAMES, count: ROLE_OUTFITS, icon: Shirt },
];

function mintCall(agent: string, traits: SkinTraits) {
  assertTestnet();
  return {
    network: "testnet" as const,
    receiver_id: PLACEHOLDER_CONTRACTS.registry,
    method_name: "forge_mint",
    args: { agent, traits },
    attached_deposit: "0",
    gas: "80000000000000",
  };
}

export default function ForgePage() {
  const seed = useMemo(() => defaultSkin("scout.vault.fe2vi.testnet"), []);
  const [traits, setTraits] = useState<SkinTraits>(seed);
  const [category, setCategory] = useState<(typeof CATEGORIES)[number]>(CATEGORIES[0]);
  const [agent, setAgent] = useState("scout.vault.fe2vi.testnet");
  const [call, setCall] = useState<string | null>(null);

  function set<K extends keyof SkinTraits>(key: K, value: number) {
    setCall(null);
    setTraits((current) => ({ ...current, [key]: value }));
  }

  const active = CATEGORIES.find((entry) => entry.key === category.key) ?? CATEGORIES[0];

  return (
    <main className="board forge-customizer">
      <h1>Skin Forge</h1>
      <p className="lede">Cosmetic only. One NEAR on testnet. Tradeable, with a 5% creator royalty.</p>
      <details className="how-mint">
        <summary>How minting works</summary>
        <p>
          The registrant funds registry escrow. Then the agent or its owner calls forge_mint with no attached deposit. The heist key cannot mint. Storage is inside the 1 NEAR. This page builds the call and does not submit it.
        </p>
      </details>
      <div className="forge-stage">
        <div className="forge-turntable">
          <SkinPreview traits={traits} />
          <TrophyShelf />
        </div>
        <Panel>
          <p className="type-label">
            {rarity(traits)} · {ROLE_NAMES[traits.role]}
          </p>
          <div className="forge-tabs" role="tablist" aria-label="Trait categories">
            {CATEGORIES.map((entry) => {
              const Icon = entry.icon;
              return (
                <button key={entry.key} type="button" role="tab" aria-selected={active.key === entry.key} onClick={() => setCategory(entry)}>
                  <Icon aria-hidden="true" strokeWidth={1.75} />
                  {entry.label}
                </button>
              );
            })}
          </div>
          <div className="forge-swatches" role="listbox" aria-label={active.label}>
            {active.names.map((name, index) => (
              <button
                key={name}
                type="button"
                role="option"
                aria-selected={traits[active.key] === index}
                onClick={() => set(active.key, index)}
              >
                {traits[active.key] === index ? <Check aria-hidden="true" strokeWidth={1.75} /> : <span className="swatch-dot" aria-hidden="true" />}
                {active.colors ? <span className="swatch" style={{ background: active.colors[index] }} /> : null}
                {name}
              </button>
            ))}
          </div>
          <label className="forge-agent">
            Agent account
            <input value={agent} onChange={(event) => { setAgent(event.target.value); setCall(null); }} spellCheck={false} />
          </label>
          <div className="forge-actions">
            <Button
              variant="secondary"
              onClick={() => {
                setCall(null);
                setTraits({
                  paint: Math.floor(Math.random() * PAINTS),
                  screen: Math.floor(Math.random() * SCREENS),
                  eyes: Math.floor(Math.random() * EYE_STYLES),
                  headwear: Math.floor(Math.random() * HEADWEAR),
                  face_item: Math.floor(Math.random() * FACE_ITEMS),
                  back_item: Math.floor(Math.random() * BACK_ITEMS),
                  emblem: Math.floor(Math.random() * EMBLEMS),
                  trail: Math.floor(Math.random() * TRAILS),
                  role: Math.floor(Math.random() * ROLE_OUTFITS),
                });
              }}
            >
              <Dices size={20} aria-hidden="true" /> Randomise
            </Button>
            <Button
              onClick={() => {
                const next = mintCall(agent.trim() || "scout.vault.fe2vi.testnet", traits);
                setCall(JSON.stringify(next, null, 2));
                window.localStorage.setItem(PREVIEW_KEY, JSON.stringify(traits));
                audio.unlock();
                audio.sting("forge");
              }}
            >
              <Hammer size={20} aria-hidden="true" /> Mint
            </Button>
          </div>
          {call ? (
            <details className="how-mint">
              <summary>forge_mint call</summary>
              <pre className="forge-call" role="status">
                {call}
              </pre>
            </details>
          ) : (
            <p className="meta">Mint builds the testnet call. Nothing is broadcast.</p>
          )}
        </Panel>
      </div>
    </main>
  );
}
