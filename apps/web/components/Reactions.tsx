"use client";

import { useEffect, useState } from "react";
import { Brain, Eye, Flame, Skull, ThumbsUp, type LucideIcon } from "lucide-react";
import {
  REACTION_KEYS,
  reactToTarget,
  type ReactionCounts,
  type ReactionKey,
  type ReactionSummary,
} from "@/lib/arena";
import { getViewerId } from "@/lib/viewer";

const OPTIONS: Array<{ key: ReactionKey; label: string; Icon: LucideIcon }> = [
  { key: "fire", label: "Fire", Icon: Flame },
  { key: "brain", label: "Big brain", Icon: Brain },
  { key: "skull", label: "Dead", Icon: Skull },
  { key: "clap", label: "Clap", Icon: ThumbsUp },
  { key: "eyes", label: "Watching", Icon: Eye },
];

function normalize(counts?: ReactionCounts): ReactionCounts {
  return REACTION_KEYS.reduce(
    (next, key) => ({ ...next, [key]: counts?.[key] ?? 0 }),
    {} as ReactionCounts,
  );
}

export function Reactions({
  targetType,
  targetId,
  counts,
  viewerReaction,
  compact = false,
}: {
  targetType: "lobby_post" | "match";
  targetId: string;
  counts?: ReactionCounts;
  viewerReaction?: ReactionKey | null;
  compact?: boolean;
}) {
  const [summary, setSummary] = useState<ReactionSummary>({
    counts: normalize(counts),
    viewerReaction: viewerReaction ?? null,
  });
  const [viewer, setViewer] = useState("");

  useEffect(() => {
    setViewer(getViewerId());
  }, []);

  useEffect(() => {
    setSummary((current) => ({
      counts: normalize(counts),
      viewerReaction: current.viewerReaction ?? viewerReaction ?? null,
    }));
  }, [counts, viewerReaction]);

  const pick = async (key: ReactionKey) => {
    if (!viewer) return;
    const previous = summary;
    const next = summary.viewerReaction === key ? "none" : key;
    const optimistic = normalize(summary.counts);
    if (summary.viewerReaction) optimistic[summary.viewerReaction] = Math.max(0, optimistic[summary.viewerReaction] - 1);
    if (next !== "none") optimistic[next] += 1;
    setSummary({ counts: optimistic, viewerReaction: next === "none" ? null : next });
    const result = await reactToTarget(targetType, targetId, next, viewer);
    if (result) setSummary(result);
    else setSummary(previous);
  };

  return (
    <div className={compact ? "reaction-bar compact" : "reaction-bar"} aria-label="Spectator reactions">
      {OPTIONS.map(({ key, label, Icon }) => {
        const active = summary.viewerReaction === key;
        return (
          <button
            key={key}
            type="button"
            className={active ? "reaction-btn on" : "reaction-btn"}
            aria-label={`${label} reaction`}
            aria-pressed={active}
            title={label}
            onClick={() => void pick(key)}
          >
            <Icon size={compact ? 14 : 16} aria-hidden="true" />
            <span>{summary.counts[key]}</span>
          </button>
        );
      })}
    </div>
  );
}
