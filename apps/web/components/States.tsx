import { Logo } from "@/components/Logo";

export function LoadingState({ hero = false, progress = null }: { label?: string; hero?: boolean; progress?: number | null }) {
  const pct = progress == null ? 0 : Math.max(0, Math.min(100, Math.round(progress)));
  return (
    <div className={hero ? "loading-screen" : "loading"} role="status" aria-live="polite">
      <div className="loading-mark">
        <Logo variant={hero ? "hero" : "nav"} />
      </div>
      {hero ? (
        <div className="load-bar" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct} aria-label="Progress">
          <span style={{ width: `${pct}%` }} />
        </div>
      ) : (
        <p className="num">{pct}%</p>
      )}
      {hero ? <p className="num">{pct}%</p> : null}
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
