export function LoadingState({ label }: { label: string }) {
  return (
    <p className="loading" role="status">
      Loading {label}…
    </p>
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
