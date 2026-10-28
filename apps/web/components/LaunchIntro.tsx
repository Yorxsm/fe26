"use client";

import { useEffect, useState } from "react";
import { Logo } from "@/components/Logo";

const SEEN = "fe2vi.launch.v1";

export function LaunchIntro() {
  const [done, setDone] = useState(false);
  useEffect(() => {
    if (window.localStorage.getItem(SEEN) === "1") {
      setDone(true);
      return;
    }
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const finish = () => {
      window.localStorage.setItem(SEEN, "1");
      setDone(true);
    };
    const timer = window.setTimeout(finish, reduce ? 400 : 5000);
    const skip = () => finish();
    window.addEventListener("pointerdown", skip);
    window.addEventListener("keydown", skip);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener("pointerdown", skip);
      window.removeEventListener("keydown", skip);
    };
  }, []);
  return (
    <main className={done ? "loading-screen" : "loading-screen loading-mark"} aria-label="fe2VI">
      <Logo variant="hero" />
    </main>
  );
}
