import type { Trust } from "@/lib/protocol";

export function TrustMarks({ trust, templateId }: { trust: Trust; templateId?: string }) {
  const template = trust.known_template
    ? templateId === "trading-cup"
      ? "Trading Cup"
      : "Heist Night"
    : "Unknown game";
  return (
    <div className="stamps" aria-label="Trust labels">
      <span className={trust.known_template ? "stamp" : "stamp warn"}>{template}</span>
      <span className={trust.delisted ? "stamp warn" : "stamp"}>{trust.delisted ? "Delisted" : "Listed"}</span>
      <span className="stamp warn">Creator text untrusted</span>
      <span className="stamp neutral">{trust.network}</span>
      <span className="stamp neutral">{trust.relay}</span>
    </div>
  );
}
