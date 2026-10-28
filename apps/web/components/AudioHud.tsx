"use client";

import { useEffect, useState } from "react";
import { Volume2, VolumeX } from "lucide-react";
import { audio } from "@/lib/audio";

export function AudioHud({ live = false }: { live?: boolean }) {
  const [muted, setMuted] = useState(true);
  const [volume, setVolume] = useState(0.7);
  const [ready, setReady] = useState(false);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const prefs = audio.hydrate();
    setVolume(prefs.volume);
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

  const silent = muted || !ready || volume === 0;

  return (
    <div className="audio-hud">
      <button
        type="button"
        className="btn btn-icon btn-secondary"
        aria-expanded={open}
        aria-label={silent ? "Sound off" : "Sound"}
        onClick={() => {
          if (!ready) {
            audio.unlock();
            audio.setMuted(false);
            setReady(true);
            setMuted(false);
          }
          setOpen((value) => !value);
        }}
      >
        {silent ? <VolumeX aria-hidden="true" strokeWidth={1.75} /> : <Volume2 aria-hidden="true" strokeWidth={1.75} />}
      </button>
      {open ? (
        <div className="audio-pop" role="group" aria-label="Volume">
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
                if (next === 0) {
                  audio.setMuted(true);
                  setMuted(true);
                  return;
                }
                audio.unlock();
                audio.setMuted(false);
                setReady(true);
                setMuted(false);
              }}
            />
          </label>
        </div>
      ) : null}
    </div>
  );
}
