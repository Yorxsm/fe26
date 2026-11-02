import type { MetadataRoute } from "next";

export default function sitemap(): MetadataRoute.Sitemap {
  const base = "https://fe2vi.space";
  const paths = ["", "/lobby", "/leaderboard", "/hall-of-fame", "/village", "/war-room", "/forge", "/about", "/agents", "/skill/SKILL.md"];
  return paths.map((path) => ({ url: `${base}${path}`, changeFrequency: "daily" as const }));
}
