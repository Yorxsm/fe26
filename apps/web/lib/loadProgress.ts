import { useProgress } from "@react-three/drei";
import { DefaultLoadingManager } from "three";

/** Touch the store so its loader hooks are installed before this module wraps them. */
void useProgress.getState();

export type LoadStep = {
  loaded: number;
  total: number;
  active: boolean;
  progress: number;
};

const steps: LoadStep[] = [];
let lastTotal = 0;
let maxProgress = 0;
let coreDone = false;
let shownFloor = 0;

export function loadShownFloor() {
  return shownFloor;
}

export function rememberLoadFloor(value: number) {
  if (value > shownFloor) shownFloor = value;
}

function publish(loaded: number, total: number, active: boolean) {
  if (coreDone) return;
  let progress = total > 0 ? Math.min(100, (loaded / total) * 100) : 0;
  progress = Math.max(maxProgress, progress);
  maxProgress = progress;
  if (!active && total > 0 && loaded >= total) {
    progress = 100;
    maxProgress = 100;
    coreDone = true;
  }
  const step: LoadStep = { loaded, total, active: coreDone ? false : active, progress };
  const prev = steps[steps.length - 1];
  if (prev && prev.loaded === loaded && prev.total === total && prev.active === active) return;
  lastTotal = total;
  steps.push(step);
  if (typeof window !== "undefined") {
    window.dispatchEvent(new CustomEvent("fe2vi-load", { detail: step }));
  }
}

const priorStart = DefaultLoadingManager.onStart;
const priorProgress = DefaultLoadingManager.onProgress;
const priorLoad = DefaultLoadingManager.onLoad;

DefaultLoadingManager.onStart = (item, loaded, total) => {
  publish(loaded, total, true);
  priorStart?.(item, loaded, total);
};

DefaultLoadingManager.onProgress = (item, loaded, total) => {
  const url = typeof item === "string" ? item : "";
  if (!coreDone && url.includes("station-core") && url.includes(".glb")) {
    publish(1, 1, false);
  } else {
    publish(loaded, total, true);
  }
  priorProgress?.(item, loaded, total);
};

DefaultLoadingManager.onLoad = () => {
  if (lastTotal > 0) publish(lastTotal, lastTotal, false);
  priorLoad?.();
};

export function pendingLoadSteps() {
  return steps.slice();
}
