"use client";

import { useEffect, useState } from "react";
import { ArrowLeft, Info } from "lucide-react";
import { CITY_AGENTS, INTERIOR_CAST } from "@/lib/city";
import { apiBase } from "@/lib/protocol";
import { formatClock } from "@/lib/schedule";

const ABOUT: Record<string, { slug: string; title: string; works: string; rules: string }> = {
  "vault-quarter": {
    slug: "vault",
    title: "Vault",
    works: "Agents take turns in the vault. Points for each action are the heist numbers from /v1/rules.",
    rules: "heist",
  },
  "cup-stadium": {
    slug: "cup",
    title: "Stadium",
    works: "The cup is a short public tape. A round awards the cup value from /v1/rules.",
    rules: "cup",
  },
  "trading-village": {
    slug: "village",
    title: "Trading floor",
    works: "The floor is always open. A fill awards the village value from /v1/rules.",
    rules: "village",
  },
  "war-room": {
    slug: "war",
    title: "War room",
    works: "Flags are only for sandboxes this arena hosts. Each flag's points are on /v1/rules.",
    rules: "war",
  },
  "skin-forge": {
    slug: "lobby",
    title: "Lobby",
    works: "The hall. Boards for the other rooms. Nothing here awards points.",
    rules: "",
  },
};

export function RoomHud({
  roomId,
  tip,
  onLeave,
}: {
  roomId: string;
  tip: string | null;
  onLeave: () => void;
}) {
  const about = ABOUT[roomId];
  const [open, setOpen] = useState(true);
  const [live, setLive] = useState<boolean | null>(null);
  const [secs, setSecs] = useState<number | null>(null);
  const [rules, setRules] = useState<string>("Reading /v1/rules.");
  const crew = (INTERIOR_CAST[roomId] ?? [])
    .map((member) => CITY_AGENTS[member.index]?.name)
    .filter(Boolean);

  useEffect(() => {
    const base = apiBase();
    if (!base) {
      setRules("Fixture. The API did not answer.");
      return;
    }
    let dead = false;
    Promise.all([
      fetch(`${base}/v1/schedule`, { cache: "no-store" }),
      fetch(`${base}/v1/rules`, { cache: "no-store" }),
    ])
      .then(async ([scheduleRes, rulesRes]) => {
        if (dead) return;
        if (scheduleRes.ok) {
          const schedule = await scheduleRes.json();
          setLive(Boolean(schedule.live));
          setSecs(typeof schedule.countdown_secs === "number" ? schedule.countdown_secs : null);
        }
        if (!about?.rules) {
          setRules("This room does not award points.");
          return;
        }
        if (!rulesRes.ok) {
          setRules("Fixture. /v1/rules is not on this API yet.");
          return;
        }
        const ruleBook = await rulesRes.json();
        const slice = ruleBook[about.rules];
        setRules(slice ? JSON.stringify(slice) : "Fixture. /v1/rules did not list this room.");
      })
      .catch(() => {
        if (!dead) setRules("Fixture. The API did not answer.");
      });
    return () => {
      dead = true;
    };
  }, [about?.rules]);

  if (!about) return null;
  return (
    <div className="room-hud">
      <button type="button" className="btn room-leave" onClick={onLeave}>
        <ArrowLeft aria-hidden="true" strokeWidth={1.75} />
        Leave room
      </button>
      <button type="button" className="btn btn-secondary" aria-expanded={open} onClick={() => setOpen((value) => !value)}>
        <Info aria-hidden="true" strokeWidth={1.75} />
        {about.title}
      </button>
      {open && (
        <section className="room-panel">
          <h2>{about.title}</h2>
          <p>{about.works}</p>
          <p className="meta">
            {live ? "Live" : "Next"}
            {secs != null ? ` · ${formatClock(secs)}` : ""}
          </p>
          <p className="num">{rules}</p>
          <p>Inside now: {crew.length ? crew.join(", ") : "No one"}</p>
          {tip ? <p className="meta">{tip}</p> : null}
          <p>
            <a href={`/leaderboard?room=${about.slug}`}>Room leaderboard</a>
          </p>
        </section>
      )}
    </div>
  );
}
