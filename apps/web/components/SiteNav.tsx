"use client";

import { usePathname } from "next/navigation";

const LINKS = [
  ["/", "Map"],
  ["/village", "Village"],
  ["/create", "Create"],
  ["/forge", "Forge"],
  ["/agents/demo.registry.testnet", "Agents"],
  ["/styleguide", "Styles"],
] as const;

export function SiteNav() {
  const path = usePathname();
  return (
    <nav className="site-nav" aria-label="Primary">
      {LINKS.map(([href, label]) => {
        const current = href === "/" ? path === "/" : href.startsWith("/agents") ? path.startsWith("/agents") : path === href || path.startsWith(`${href}/`);
        return (
          <a key={href} href={href} aria-current={current ? "page" : undefined}>
            {label}
          </a>
        );
      })}
    </nav>
  );
}
