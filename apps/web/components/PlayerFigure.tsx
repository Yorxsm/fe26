import { playerMark } from "@/lib/passports";

export function PlayerFigure({ tokenId, tier, large }: { tokenId: string; tier: string | null; large?: boolean }) {
  const mark = playerMark(tokenId, tier);
  const label = `${tokenId}${tier ? `, ${tier}` : ""}`;
  return (
    <svg
      className={large ? "player-figure hero" : "player-figure"}
      viewBox="0 0 120 176"
      role="img"
      aria-label={label}
    >
      <rect x="28" y="46" width="64" height="70" rx="10" fill={mark.jersey} />
      <rect x="28" y="46" width="64" height="10" fill={mark.trim} />
      <circle cx="60" cy="28" r="16" fill="#d4d2cc" />
      {mark.visor ? <rect x="48" y="24" width="24" height="6" rx="2" fill="#141416" /> : null}
      <text x="60" y="88" textAnchor="middle" fill="#141416" fontSize="20" fontFamily="ui-monospace, monospace">
        {mark.number}
      </text>
      <rect x="18" y="58" width="12" height="40" rx="4" fill={mark.jersey} />
      <rect x="90" y="58" width="12" height="40" rx="4" fill={mark.jersey} />
      <rect x="38" y="112" width="16" height="40" rx="4" fill="#222225" />
      <rect x="66" y="112" width="16" height="40" rx="4" fill="#222225" />
      <rect x="34" y="148" width="22" height="10" rx="2" fill={mark.boots} />
      <rect x="64" y="148" width="22" height="10" rx="2" fill={mark.boots} />
    </svg>
  );
}
