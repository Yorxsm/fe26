"use client";

import { useState } from "react";
import { Eye, Hammer, MessageSquare, MousePointer2, Sparkles, Volume2, X } from "lucide-react";
import { Logo } from "@/components/Logo";
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

const SWATCHES: { hex: string; name: string; ink?: string }[] = [
  { hex: "#0a0a0b", name: "background" },
  { hex: "#141416", name: "panel" },
  { hex: "#1e1e21", name: "raised" },
  { hex: "#c94a1a", name: "orange", ink: "#0a0a0b" },
  { hex: "#8a3110", name: "lip" },
  { hex: "#2fb86a", name: "green", ink: "#0a0a0b" },
  { hex: "#d4d2cc", name: "text", ink: "#0a0a0b" },
  { hex: "#8b8a86", name: "secondary", ink: "#0a0a0b" },
  { hex: "#222225", name: "secondary button" },
  { hex: "#121214", name: "secondary lip" },
  { hex: "#1f1f23", name: "line" },
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
        <Logo variant="hero" />
        <h1>Iron plate</h1>
        <p className="lede">Muted core colours, Chakra Petch for display, Inter for body. Eight-pixel grid, 24px margins, twelve columns.</p>
      </header>

      <section>
        <h2>Type</h2>
        <p className="type-display">Display 48</p>
        <p className="type-h1">Heading 1 · 32</p>
        <p className="type-h2">Heading 2 · 24</p>
        <p className="type-h3">Heading 3 · 20</p>
        <p>Body 16. Line height 1.5. Letter-spacing stays at zero.</p>
        <p className="meta">Small 14 for notes and hashes.</p>
        <p className="label">Caption 12 · tracking +0.04em</p>
        <p className="num live-ink">
          <Tick value={26} /> agents · JetBrains Mono
        </p>
      </section>

      <section>
        <h2>Colour</h2>
        <div className="swatches">
          {SWATCHES.map((swatch) => (
            <span key={swatch.hex} style={{ background: swatch.hex, color: swatch.ink ?? "#d4d2cc" }}>
              {swatch.name}
              <small>{swatch.hex}</small>
            </span>
          ))}
        </div>
        <p className="meta">
          #d4d2cc on #0a0a0b is 13.1:1. #d4d2cc on #141416 is 12.2:1. #8b8a86 on #141416 is 5.3:1. #0a0a0b on #c94a1a is 4.2:1.
        </p>
      </section>

      <section>
        <h2>Buttons</h2>
        <div className="row">
          <Button>
            <Eye aria-hidden="true" strokeWidth={1.75} />
            Primary
          </Button>
          <Button variant="secondary">
            <MousePointer2 aria-hidden="true" strokeWidth={1.75} />
            Secondary
          </Button>
          <Button className="is-hover">
            <Sparkles aria-hidden="true" strokeWidth={1.75} />
            Hover
          </Button>
          <Button className="is-pressed">
            <Hammer aria-hidden="true" strokeWidth={1.75} />
            Pressed
          </Button>
          <Button disabled>
            <Hammer aria-hidden="true" strokeWidth={1.75} />
            Disabled
          </Button>
          <Button className="btn-sm">
            <Eye aria-hidden="true" strokeWidth={1.75} />
            Small
          </Button>
          <Button>
            <Hammer aria-hidden="true" strokeWidth={1.75} />
            With icon
          </Button>
          <IconButton icon={Volume2} label="Sound" />
          <Tooltip tip="The lip is a solid darker shade.">
            <Button variant="ghost">
              <Eye aria-hidden="true" strokeWidth={1.75} />
              Hover tip
            </Button>
          </Tooltip>
        </div>
      </section>

      <section className="guide-split">
        <Panel title="Panel">
          <p>Flat plate, one-pixel neutral border, 8px radius. Selected state is a fill change.</p>
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
              Caps hold.{" "}
              <button type="button" className="btn btn-ghost btn-sm" onClick={() => setToast(false)}>
                <X aria-hidden="true" strokeWidth={1.75} />
                Dismiss
              </button>
            </Toast>
          ) : (
            <Button variant="secondary" onClick={() => setToast(true)}>
              <MessageSquare aria-hidden="true" strokeWidth={1.75} />
              Show toast
            </Button>
          )}
        </div>
      </section>

      <section>
        <h2>Icons</h2>
        <IconRow />
      </section>
    </main>
  );
}
