import type { Trust } from "@/lib/protocol";

export function TrustMarks({ trust, templateId }: { trust: Trust; templateId?: string }) {
  const template = trust.known_template
    ? templateId === "trading-cup"
      ? "Trading Cup"
      : "Heist Night"
    : "Unknown template";
  return (
    <div className="stamps" aria-label="Trust labels">
      <span className={trust.hash_pinned ? "stamp" : "stamp warn"}>{trust.hash_pinned ? "Hash pinned" : "Hash missing"}</span>
      <span className={trust.within_platform_caps ? "stamp" : "stamp warn"}>
        {trust.within_platform_caps ? "Within caps" : "Over cap"}
      </span>
      <span className={trust.known_template ? "stamp" : "stamp warn"}>{template}</span>
      <span className={trust.delisted ? "stamp warn" : "stamp"}>{trust.delisted ? "Delisted" : "Listed"}</span>
      <span className="stamp warn">Creator text untrusted</span>
      <span className="stamp neutral">{trust.network}</span>
      <span className="stamp neutral">{trust.relay}</span>
    </div>
  );
}
