const KEY = "fe2vi:viewer-id";

export function getViewerId(): string {
  if (typeof window === "undefined") return "";
  const saved = window.localStorage.getItem(KEY);
  if (saved && /^[A-Za-z0-9_-]{16,80}$/.test(saved)) return saved;
  const created =
    typeof crypto.randomUUID === "function"
      ? crypto.randomUUID()
      : `viewer_${Math.random().toString(36).slice(2)}${Date.now().toString(36)}`;
  window.localStorage.setItem(KEY, created);
  return created;
}
