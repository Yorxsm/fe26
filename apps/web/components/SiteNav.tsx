"use client";

import { useState } from "react";
import { usePathname } from "next/navigation";
import { DoorOpen, Home, Map, Menu, Trophy, Users, X, type LucideIcon } from "lucide-react";

const LINKS: Array<readonly [string, string, LucideIcon]> = [
  ["/#title", "Home", Home],
  ["/leaderboard", "Board", Trophy],
  ["/village", "Village", Map],
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
          const current = href === "/#title" ? path === "/" : href.startsWith("/agents") ? path.startsWith("/agents") : path === href || path.startsWith(`${href}/`);
          return (
            <a
              key={href}
              href={href}
              aria-current={current ? "page" : undefined}
              onClick={(event) => {
                setOpen(false);
                if (href !== "/#title") return;
                window.dispatchEvent(new Event("fe2vi-title"));
                if (window.location.pathname === "/" && window.location.hash === "#title") event.preventDefault();
              }}
            >
              <Icon aria-hidden="true" strokeWidth={1.75} />
              {label}
            </a>
          );
        })}
      </nav>
    </div>
  );
}
