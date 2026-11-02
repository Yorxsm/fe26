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

function publish(loaded: number, total: number, active: boolean) {
  const progress = total > 0 ? Math.min(100, (loaded / total) * 100) : 0;
  const step: LoadStep = { loaded, total, active, progress };
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
  publish(loaded, total, true);
  priorProgress?.(item, loaded, total);
};

DefaultLoadingManager.onLoad = () => {
  if (lastTotal > 0) publish(lastTotal, lastTotal, false);
  priorLoad?.();
};

export function pendingLoadSteps() {
  return steps.slice();
}
