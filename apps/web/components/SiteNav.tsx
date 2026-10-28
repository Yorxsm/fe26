"use client";

import { useState } from "react";
import { usePathname } from "next/navigation";
import { DoorOpen, Home, Map, Menu, Users, X, type LucideIcon } from "lucide-react";

const LINKS: Array<readonly [string, string, LucideIcon]> = [
  ["/", "Map", Map],
  ["/village", "Village", Home],
  ["/lobby", "Lobby", DoorOpen],
  ["/agents/demo.registry.testnet", "Agents", Users],
];

export function SiteNav() {
  const path = usePathname();
  const [open, setOpen] = useState(false);
  return (
    <div className="nav-wrap">
      <button type="button" className="nav-toggle" aria-expanded={open} aria-label={open ? "Close menu" : "Open menu"} onClick={() => setOpen((value) => !value)}>
        {open ? <X aria-hidden="true" strokeWidth={1.75} /> : <Menu aria-hidden="true" strokeWidth={1.75} />}
      </button>
      <nav className={open ? "site-nav open" : "site-nav"} aria-label="Primary">
        {LINKS.map(([href, label, Icon]) => {
          const current = href === "/" ? path === "/" : href.startsWith("/agents") ? path.startsWith("/agents") : path === href || path.startsWith(`${href}/`);
          return (
            <a key={href} href={href} aria-current={current ? "page" : undefined} onClick={() => setOpen(false)}>
              <Icon aria-hidden="true" strokeWidth={1.75} />
              {label}
            </a>
          );
        })}
      </nav>
    </div>
  );
}
