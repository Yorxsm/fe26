import { Logo } from "@/components/Logo";

function stageFor(pct: number) {
  if (pct < 16) return "Lighting the street lamps";
  if (pct < 38) return "Laying the river and roads";
  if (pct < 62) return "Hanging the room signs";
  if (pct < 84) return "Waking the agents";
  if (pct < 99) return "Opening the gates";
  return "Ready";
}

export function LoadingState({ hero = false, progress = null, label }: { label?: string; hero?: boolean; progress?: number | null }) {
  const pct = progress == null ? 0 : Math.max(0, Math.min(100, Math.round(progress)));
  if (!hero) {
    return (
      <div className="loading" role="status" aria-live="polite">
        <div className="loading-mark">
          <Logo variant="nav" />
        </div>
        <p className="num">{pct}%</p>
      </div>
    );
  }
  return (
    <div className="loading-screen" role="status" aria-live="polite" aria-label={`Loading ${label ?? "fe2VI"}`}>
      <div className="loading-shell">
        <p className="loading-kicker">
          <span className="live-dot" aria-hidden="true" />
          City link
        </p>
        <div className="loading-mark">
          <Logo variant="hero" />
        </div>
        <div className="loading-copy">
          <p className="loading-title">{label ?? "the map"}</p>
          <p className="loading-stage">{stageFor(pct)}</p>
        </div>
        <div className="load-bar" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct} aria-label="Progress">
          <span style={{ transform: `scaleX(${pct / 100})` }} />
        </div>
        <div className="loading-foot">
          <span className="num">{pct}%</span>
          <span className="loading-rooms">Vault · Cup · Floor · War · Forge</span>
        </div>
      </div>
    </div>
  );
}

export function EmptyState({ title, detail }: { title: string; detail: string }) {
  return (
    <div className="empty">
      <h2>{title}</h2>
      <p>{detail}</p>
    </div>
  );
}

export function ErrorState({ title, detail }: { title: string; detail: string }) {
  return (
    <div className="error" role="alert">
      <h2>{title}</h2>
      <p>{detail}</p>
    </div>
  );
}
