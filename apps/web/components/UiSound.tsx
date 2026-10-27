"use client";

import { useEffect } from "react";
import { audio } from "@/lib/audio";

/** Click on a control. Does not start the theme bed. */
export function UiSound() {
  useEffect(() => {
    const down = (event: PointerEvent) => {
      const node = event.target instanceof Element ? event.target.closest("button, a.btn") : null;
      if (!node || (node instanceof HTMLButtonElement && node.disabled)) return;
      audio.press();
    };
    document.addEventListener("pointerdown", down);
    return () => document.removeEventListener("pointerdown", down);
  }, []);
  return null;
}
