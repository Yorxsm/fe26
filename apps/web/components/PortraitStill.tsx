"use client";

import { useEffect, useState } from "react";
import { renderPortrait } from "@/lib/portraitCapture";
import type { SkinTraits } from "@/lib/skin";

export function PortraitStill({ traits, fallback }: { traits: SkinTraits; fallback?: string | null }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    setSrc(null);
    renderPortrait(traits).then((url) => {
      if (!alive) return;
      setSrc(url.startsWith("data:image/") ? url : "");
    });
    return () => {
      alive = false;
    };
  }, [
    traits.paint,
    traits.screen,
    traits.eyes,
    traits.headwear,
    traits.face_item,
    traits.back_item,
    traits.emblem,
    traits.trail,
    traits.role,
  ]);
  if (src === null) return <span className="passport-still" aria-hidden="true" />;
  if (!src && fallback) return <img className="passport-still" alt="" src={fallback} />;
  if (!src) return <span className="passport-still" aria-hidden="true" />;
  return <img className="passport-still" alt="" src={src} />;
}
