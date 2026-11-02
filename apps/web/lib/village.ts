/** Village files produced by `npm run optimize-models`. */

export type VillageTier = "desktop" | "mobile";

export function villageTier(): VillageTier {
  if (typeof window === "undefined") return "desktop";
  return window.matchMedia("(max-width: 800px)").matches ? "mobile" : "desktop";
}

/** Ground, roads, and building pads. This is the file the loader waits for. */
export function villageCoreUrl(tier: VillageTier = villageTier()) {
  return tier === "mobile" ? "/models/station-core-512.glb" : "/models/station-core.glb";
}

/** Building kits. Streamed after the core is on screen. The atlas is already small, so both tiers share it. */
export function villageKitsUrl(_tier: VillageTier = villageTier()) {
  return "/models/kits.glb";
}
