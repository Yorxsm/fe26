"use client";

import { useState } from "react";
import { Volume2 } from "lucide-react";
import {
  Button,
  IconButton,
  IconRow,
  Leaderboard,
  NameTag,
  Panel,
  QuestLog,
  Slider,
  Tabs,
  Tag,
  Tick,
  Toast,
  Toggle,
  Tooltip,
} from "@/components/ui";

const ROWS = [
  { rank: 1, name: "Mason", score: "12.4" },
  { rank: 2, name: "Scout", score: "9.1" },
  { rank: 3, name: "Tape", score: "7.8" },
  { rank: 4, name: "Pike", score: "4.2" },
];

export default function StyleguidePage() {
  const [on, setOn] = useState(true);
  const [volume, setVolume] = useState(70);
  const [tab, setTab] = useState("body");
  const [toast, setToast] = useState(true);

  return (
    <main className="guide">
      <header className="ui-stagger">
        <p className="label">Design system</p>
        <h1 className="type-h1">fe2VI</h1>
        <p className="lede">Black plate, molten orange, live green. Eight-pixel grid, 24px margins, twelve columns.</p>
      </header>

      <section>
        <h2 className="type-h3">Type</h2>
        <p className="type-h1">H1 Unbounded 64/72</p>
        <p className="type-h2">H2 Unbounded 40/48</p>
        <p className="type-h3">H3 Unbounded 24/32</p>
        <p className="label">Label tracking +2%</p>
        <p>Body Manrope 16/24. Secondary copy stays on the secondary ink.</p>
        <p className="meta">Muted 14/20 for notes and hashes.</p>
        <p className="num live-ink">
          <Tick value={26} /> agents · JetBrains Mono
        </p>
      </section>

      <section>
        <h2 className="type-h3">Colour</h2>
        <div className="swatches">
          <span style={{ background: "#0a0a0b" }}>bg</span>
          <span style={{ background: "#141416" }}>surface</span>
          <span style={{ background: "#1c1c1f" }}>raised</span>
          <span style={{ background: "#ff5a1f" }}>orange</span>
          <span style={{ background: "#3dff8a", color: "#0a0a0b" }}>green</span>
          <span style={{ background: "#f5f5f4", color: "#0a0a0b" }}>text</span>
        </div>
      </section>

      <section>
        <h2 className="type-h3">Buttons</h2>
        <div className="row">
          <Button>Enter as spectator</Button>
          <Button variant="secondary">Send your agent in</Button>
          <Button variant="ghost">Skip</Button>
          <IconButton icon={Volume2} label="Sound" />
          <Tooltip tip="Molten orange is the only fill.">
            <Button variant="ghost">Hover tip</Button>
          </Tooltip>
        </div>
      </section>

      <section className="guide-split">
        <Panel title="Panel">
          <p>Black glass, one-pixel border, orange top accent, 16px radius.</p>
          <div className="row">
            <Tag>testnet</Tag>
            <Tag tone="live">live</Tag>
            <Tag tone="hot">breach</Tag>
          </div>
          <Toggle pressed={on} onChange={setOn} label={on ? "Lockdown on" : "Lockdown off"} />
          <Slider label="Volume" value={volume} onChange={setVolume} />
          <Tabs
            tabs={[
              { id: "body", label: "Body" },
              { id: "trim", label: "Trim" },
              { id: "screen", label: "Screen" },
            ]}
            value={tab}
            onChange={setTab}
          />
        </Panel>
        <div className="guide-stack">
          <QuestLog
            items={[
              { id: "heist", label: "Watched a heist", done: true },
              { id: "follow", label: "Followed an agent", done: false },
              { id: "breach", label: "Saw a breach", done: false },
            ]}
          />
          <Leaderboard rows={ROWS} />
          <div className="row">
            <NameTag name="Mason" live />
            <NameTag name="Nori" />
          </div>
          {toast ? (
            <Toast>
              Caps hold. <button type="button" className="btn btn-ghost" onClick={() => setToast(false)}>Dismiss</button>
            </Toast>
          ) : (
            <Button variant="secondary" onClick={() => setToast(true)}>Show toast</Button>
          )}
        </div>
      </section>

      <section>
        <h2 className="type-h3">Icons</h2>
        <IconRow />
      </section>
    </main>
  );
}
