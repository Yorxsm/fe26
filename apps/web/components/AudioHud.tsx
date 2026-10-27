"use client";

import { useEffect, useState } from "react";
import { Volume1, Volume2, VolumeX } from "lucide-react";
import { audio } from "@/lib/audio";

export function AudioHud({ live = false }: { live?: boolean }) {
  const [muted, setMuted] = useState(true);
  const [volume, setVolume] = useState(0.7);
  const [quiet, setQuiet] = useState(false);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    const prefs = audio.hydrate();
    setVolume(prefs.volume);
    setQuiet(prefs.quiet);
    if (live || audio.running()) {
      setReady(true);
      setMuted(prefs.muted);
    }
    let last: EventTarget | null = null;
    const over = (event: PointerEvent) => {
      const node = event.target instanceof Element ? event.target.closest("button, a") : null;
      if (!node || node === last) return;
      last = node;
      audio.hover();
    };
    document.addEventListener("pointerover", over);
    return () => {
      document.removeEventListener("pointerover", over);
    };
  }, [live]);

  return (
    <div className="audio-hud" role="group" aria-label="Sound">
      <button
        type="button"
        aria-pressed={!muted && ready}
        onClick={() => {
          if (!ready || muted) {
            audio.unlock();
            audio.setMuted(false);
            setReady(true);
            setMuted(false);
            return;
          }
          audio.setMuted(true);
          setMuted(true);
        }}
      >
        {muted || !ready ? <Volume2 aria-hidden="true" strokeWidth={1.75} /> : <VolumeX aria-hidden="true" strokeWidth={1.75} />}
        {muted || !ready ? "Unmute" : "Mute"}
      </button>
      <label>
        Volume
        <input
          type="range"
          min={0}
          max={1}
          step={0.05}
          value={volume}
          style={{ ["--pct" as string]: `${Math.round(volume * 100)}%` }}
          aria-valuetext={`${Math.round(volume * 100)} percent`}
          onChange={(event) => {
            const next = Number(event.target.value);
            setVolume(next);
            audio.setVolume(next);
          }}
        />
      </label>
      <button
        type="button"
        className="btn-quiet"
        aria-pressed={quiet}
        onClick={() => {
          const next = !quiet;
          setQuiet(next);
          audio.setQuiet(next);
        }}
      >
        <Volume1 aria-hidden="true" strokeWidth={1.75} />
        {quiet ? "Quiet on" : "Quiet"}
      </button>
    </div>
  );
}
