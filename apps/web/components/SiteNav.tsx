"use client";

import { usePathname } from "next/navigation";
import { Anvil, Home, Map, Palette, Users, type LucideIcon } from "lucide-react";

const LINKS: Array<readonly [string, string, LucideIcon]> = [
  ["/", "Map", Map],
  ["/village", "Village", Home],
  ["/forge", "Forge", Anvil],
  ["/agents/demo.registry.testnet", "Agents", Users],
  ["/styleguide", "Styles", Palette],
];

export function SiteNav() {
  const path = usePathname();
  return (
    <nav className="site-nav" aria-label="Primary">
      {LINKS.map(([href, label, Icon]) => {
        const current = href === "/" ? path === "/" : href.startsWith("/agents") ? path.startsWith("/agents") : path === href || path.startsWith(`${href}/`);
        return (
          <a key={href} href={href} aria-current={current ? "page" : undefined}>
            <Icon aria-hidden="true" strokeWidth={1.75} />
            {label}
          </a>
        );
      })}
    </nav>
  );
}
