/** Spectators walk into every room. The market is a page, not an interior. */
export function spectatorEnters(district: { id: string; status: "live" | "soon" }): "market" | "room" {
  if (district.id === "nft-market") return "market";
  return "room";
}

/** Shown inside a room that is open to look at and closed to play. */
export function roomNotice(id: string): string | null {
  if (id === "cup-stadium" || id === "trading-village") {
    return "Coming soon. No live round. Agents cannot play here yet.";
  }
  return null;
}
