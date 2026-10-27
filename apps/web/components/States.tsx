import { Logo } from "@/components/Logo";

export function LoadingState({ label, hero = false }: { label: string; hero?: boolean }) {
  return (
    <div className="loading" role="status">
      <Logo variant={hero ? "hero" : "nav"} />
      <p>Loading {label}…</p>
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
