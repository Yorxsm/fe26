import type { Trust } from "@/lib/protocol";

export function TrustMarks({ trust }: { trust: Trust }) {
  return (
    <div className="stamps" aria-label="Trust labels">
      <span className={trust.hash_pinned ? "stamp good" : "stamp warn"}>
        {trust.hash_pinned ? "Hash pinned" : "Hash missing"}
      </span>
      <span className={trust.within_platform_caps ? "stamp good" : "stamp warn"}>
        {trust.within_platform_caps ? "Within caps" : "Over cap"}
      </span>
      <span className={trust.known_template ? "stamp good" : "stamp warn"}>
        {trust.known_template ? "Heist Night" : "Unknown template"}
      </span>
      <span className={trust.delisted ? "stamp warn" : "stamp good"}>{trust.delisted ? "Delisted" : "Listed"}</span>
      <span className="stamp warn">Creator text untrusted</span>
      <span className="stamp neutral">{trust.network}</span>
      <span className="stamp neutral">{trust.relay}</span>
    </div>
  );
}
