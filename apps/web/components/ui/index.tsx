"use client";

import { useEffect, useState, type ButtonHTMLAttributes, type ReactNode } from "react";
import {
  Anvil,
  Bot,
  Compass,
  Dices,
  Eye,
  Gamepad2,
  Hammer,
  Home,
  Lock,
  Radio,
  ScrollText,
  Send,
  ShieldAlert,
  SkipForward,
  Swords,
  Users,
  Volume1,
  Volume2,
  VolumeX,
  X,
  type LucideIcon,
} from "lucide-react";

type ButtonVariant = "primary" | "secondary" | "ghost" | "icon";

export function Button({
  variant = "primary",
  children,
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: ButtonVariant }) {
  return (
    <button type="button" className={`btn btn-${variant} ${className}`.trim()} {...props}>
      {children}
    </button>
  );
}

export function IconButton({
  icon: Icon,
  label,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { icon: LucideIcon; label: string }) {
  return (
    <button type="button" className="btn btn-icon" aria-label={label} {...props}>
      <Icon size={20} strokeWidth={1.75} aria-hidden="true" />
    </button>
  );
}

export function Panel({
  title,
  children,
  className = "",
}: {
  title?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={`panel ui-in ${className}`.trim()}>
      {title ? <h2 className="label">{title}</h2> : null}
      <div className="ui-stagger">{children}</div>
    </section>
  );
}

export function Tag({ children, tone = "neutral" }: { children: ReactNode; tone?: "neutral" | "live" | "hot" }) {
  return <span className={`tag tag-${tone}`}>{children}</span>;
}

export function Toggle({
  pressed,
  onChange,
  label,
}: {
  pressed: boolean;
  onChange: (next: boolean) => void;
  label: string;
}) {
  return (
    <button type="button" className="toggle" aria-pressed={pressed} onClick={() => onChange(!pressed)}>
      <span className="toggle-track" aria-hidden="true">
        <span className="toggle-knob" />
      </span>
      {label}
    </button>
  );
}

export function Slider({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (next: number) => void;
}) {
  return (
    <label className="slider">
      <span>{label}</span>
      <input
        type="range"
        min={0}
        max={100}
        value={value}
        style={{ ["--pct" as string]: `${value}%` }}
        onChange={(event) => onChange(Number(event.target.value))}
      />
      <span className="num">{value}</span>
    </label>
  );
}

export function Tabs({
  tabs,
  value,
  onChange,
}: {
  tabs: { id: string; label: string }[];
  value: string;
  onChange: (id: string) => void;
}) {
  return (
    <div className="tabs" role="tablist">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          type="button"
          role="tab"
          aria-selected={value === tab.id}
          className={value === tab.id ? "on" : ""}
          onClick={() => onChange(tab.id)}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}

export function Toast({ children }: { children: ReactNode }) {
  return (
    <div className="toast ui-in" role="status">
      {children}
    </div>
  );
}

export function Tooltip({ tip, children }: { tip: string; children: ReactNode }) {
  return (
    <span className="tip" data-tip={tip} tabIndex={0}>
      {children}
    </span>
  );
}

export function QuestLog({ items }: { items: { id: string; label: string; done: boolean }[] }) {
  return (
    <Panel title="Quest log" className="quest-panel">
      <ul className="quest-items">
        {items.map((item) => (
          <li key={item.id}>
            <span className={item.done ? "live-dot" : "dot-idle"} aria-hidden="true" />
            {item.label}
            <span className="sr-only">{item.done ? ", done" : ", open"}</span>
          </li>
        ))}
      </ul>
    </Panel>
  );
}

export function Leaderboard({
  rows,
}: {
  rows: { rank: number; name: string; score: string }[];
}) {
  return (
    <table className="board">
      <caption className="label">Leaderboard</caption>
      <thead>
        <tr>
          <th scope="col">Rank</th>
          <th scope="col">Agent</th>
          <th scope="col">Score</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((row) => (
          <tr key={row.rank}>
            <td className="num">{row.rank}</td>
            <td>{row.name}</td>
            <td className="num">{row.score}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export function NameTag({ name, live = false }: { name: string; live?: boolean }) {
  return (
    <span className="name-tag">
      {live ? <span className="live-dot" aria-hidden="true" /> : <span className="dot-idle" aria-hidden="true" />}
      {name}
      {live ? <span className="sr-only">, live</span> : null}
    </span>
  );
}

export function Tick({ value }: { value: number }) {
  const [shown, setShown] = useState(0);
  useEffect(() => {
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduce) {
      setShown(value);
      return;
    }
    const start = performance.now();
    let frame = 0;
    const step = (now: number) => {
      const t = Math.min(1, (now - start) / 700);
      setShown(Math.round(value * (1 - (1 - t) ** 3)));
      if (t < 1) frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  }, [value]);
  return <>{shown}</>;
}

export function IconRow() {
  const icons = [
    { icon: Eye, label: "Spectator" },
    { icon: Send, label: "Send" },
    { icon: SkipForward, label: "Skip" },
    { icon: Volume2, label: "Unmute" },
    { icon: VolumeX, label: "Mute" },
    { icon: Volume1, label: "Quiet" },
    { icon: Gamepad2, label: "Games" },
    { icon: ShieldAlert, label: "Audit call" },
    { icon: Lock, label: "Lockdown" },
    { icon: Dices, label: "Randomise" },
    { icon: Hammer, label: "Mint" },
    { icon: Home, label: "Village" },
    { icon: Anvil, label: "Forge" },
    { icon: Users, label: "Agents" },
    { icon: Bot, label: "Agent" },
    { icon: X, label: "Dismiss" },
    { icon: Swords, label: "Match" },
    { icon: ScrollText, label: "Quest log" },
    { icon: Radio, label: "Live" },
    { icon: Compass, label: "Districts" },
  ];
  return (
    <div className="icon-row" aria-label="Icons">
      {icons.map(({ icon: Icon, label }) => (
        <span key={label} className="icon-sample" title={label}>
          <Icon size={20} strokeWidth={1.75} aria-hidden="true" />
          <span>{label}</span>
        </span>
      ))}
    </div>
  );
}

export function Minimap({
  spots,
  active,
  onPick,
}: {
  spots: { id: string; name: string; x: number; z: number }[];
  active: string | null;
  onPick?: (id: string) => void;
}) {
  return (
    <div className="minimap" role="group" aria-label="Districts">
      {spots.map((spot) => {
        const left = ((spot.x + 70) / 140) * 100;
        const top = ((spot.z + 55) / 110) * 100;
        return (
          <button
            key={spot.id}
            type="button"
            className={active === spot.id ? "on" : ""}
            style={{ left: `${left}%`, top: `${top}%` }}
            aria-label={spot.name}
            aria-pressed={active === spot.id}
            onClick={() => onPick?.(spot.id)}
          />
        );
      })}
    </div>
  );
}
