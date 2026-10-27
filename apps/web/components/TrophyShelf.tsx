import { ArrowLeftRight, Calendar, KeyRound, Lock, Percent, ShieldAlert, Trophy } from "lucide-react";
import { DEMO_SKIN_STATS, EARNED_TRAIT_SPECS, type SkinStats } from "@/lib/skin";

export function RoyaltyLine() {
  return (
    <p className="royalty-line">
      <ArrowLeftRight aria-hidden="true" strokeWidth={1.75} />
      Tradeable
      <span aria-hidden="true">·</span>
      <Percent aria-hidden="true" strokeWidth={1.75} />
      5% creator royalty
    </p>
  );
}

export function TrophyShelf({ stats = DEMO_SKIN_STATS }: { stats?: SkinStats }) {
  const counters = [
    { icon: Trophy, label: "wins", value: stats.wins },
    { icon: KeyRound, label: "heists cracked", value: stats.heistsCracked },
    { icon: ShieldAlert, label: "times caught", value: stats.timesCaught },
    { icon: Calendar, label: "seasons", value: stats.seasonsPlayed },
  ];
  return (
    <section id="trophy-shelf" className="trophy-shelf" aria-label="Trophy shelf">
      <h3>Trophy shelf</h3>
      <p className="meta">Cosmetic record on the skin token. A trade keeps it. It does not change score or caps.</p>
      <RoyaltyLine />
      <ul className="stat-row">
        {counters.map((entry) => {
          const Icon = entry.icon;
          return (
            <li key={entry.label} className="chip">
              <Icon aria-hidden="true" strokeWidth={1.75} />
              <span className="num">{entry.value}</span>
              {entry.label}
            </li>
          );
        })}
      </ul>
      <ul id="earned-traits" className="earned-traits">
        {EARNED_TRAIT_SPECS.map((spec) => {
          const progress = Math.min(spec.goal, spec.progressOf(stats));
          const unlocked = progress >= spec.goal;
          return (
            <li key={spec.id} className={unlocked ? "earned-trait" : "earned-trait locked"}>
              {unlocked ? null : <Lock aria-hidden="true" strokeWidth={1.75} />}
              <span>{spec.name}</span>
              <span className="num">
                {progress}/{spec.goal} {spec.unit}
              </span>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
