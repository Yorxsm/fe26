export type SeasonBoardRow = {
  label: string;
  value?: string;
  tone?: "green" | "orange";
};

export type SeasonRank = { name?: string; points?: number; rank?: number };

/** `null` means the leaderboard request failed. An empty array is a live board with nobody on it. */
export function seasonBoardRows(rows: SeasonRank[] | null): SeasonBoardRow[] {
  if (rows === null) return [{ label: "LIVE DATA UNAVAILABLE" }];
  const next = rows.slice(0, 4).map((row) => ({
    label: `${row.rank ?? ""}  ${String(row.name ?? "").toUpperCase()}`.trim(),
    value: String(row.points ?? ""),
    tone: "orange" as const,
  }));
  return next.length ? next : [{ label: "NO POINTS YET" }];
}
