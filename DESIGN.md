---
version: alpha
name: fe2VI iron broadcast
description: Live arena surfaces adapted from a sports gamecast, recast as an iron foundry scoreboard. ESPN is the reference for density only. fe2VI does not use that name, logo, or palette.
colors:
  background: "#121418"
  surface: "#1c2128"
  surface-raised: "#262d36"
  ink: "#f2f4f6"
  ink-secondary: "#d5dde4"
  ink-muted: "#c5ced6"
  forge: "#e8a87c"
  action: "#9a3412"
  action-ink: "#ffffff"
  focus: "#f2c14e"
  stamp-paper: "#f4e1c1"
  stamp-ink: "#3a2414"
  danger-surface: "#3a1614"
  danger-ink: "#ffd0c8"
  ok-surface: "#123028"
  ok-ink: "#b6f0d2"
  line: "#3a424d"
typography:
  display:
    fontFamily: Fraunces
    fontWeight: 600
    fontSize: 2.4rem
    lineHeight: 1.15
  body:
    fontFamily: IBM Plex Mono
    fontWeight: 400
    fontSize: 0.95rem
    lineHeight: 1.5
  meta:
    fontFamily: IBM Plex Mono
    fontWeight: 500
    fontSize: 0.78rem
    lineHeight: 1.4
spacing:
  xs: 4px
  sm: 8px
  md: 16px
  lg: 24px
  xl: 40px
radius:
  sharp: 2px
  stamp: 2px
motion:
  meter: 600ms
  easing: ease-out
---

# fe2VI iron broadcast

## Overview

The reference is a live sports gamecast: a status strip, a clock, two competitor cards, a score, and a play-by-play that updates while you watch. fe2VI keeps that density and drops the broadcast brand. The arena is an iron foundry. Surfaces are dark plate. The one hot color is forge orange, used for the action button and the pot meter. Stamps are paper labels pressed onto the plate so trust state is readable at a glance.

YAML tokens above are normative. The sections below say why a token exists. Components in `apps/web` use these values and no others.

## Colors

`background` is the page. `surface` is a card. `surface-raised` is a seat or a meter track. `ink` is primary text. `ink-secondary` is body copy on the dark plate and stays above WCAG AA. `ink-muted` is for timestamps and hashes, still AA on `surface`.

`action` is the only filled button color. Its label is `action-ink` (white). `forge` is the meter fill and the brand numeral, never body text on `background`. `focus` is the keyboard ring.

Danger and success are dark fields with light text. Pale text on a pale chip is not used.

## Typography

Fraunces 600 is display only: the wordmark, page titles, and the pot figure. IBM Plex Mono 400 is UI and body. IBM Plex Mono 500 is stamps, meta, and the round clock. Both families are shipped with the app. The build does not request a font from a third party.

## Layout

The page is a single column until 960px, then the lobby becomes a two-column grid and the live game places seats beside the feed. Spacing steps are 4, 8, 16, 24, and 40. Content width stops at 1080px. The status strip sticks under the wordmark and names the network.

## Elevation & Depth

Depth is a 1px `line` border plus a 2px offset shadow in `background`. There is no blur and no floating card stack. The pot meter is the only element that rises, and it rises by fill, not by shadow.

## Shapes

Radius is 2px. Stamps keep a clipped corner via a polygon, so they read as pressed labels rather than pills. Buttons use the same 2px radius.

## Components

- **Status strip.** Network, relay or fixture, and whether chain events are verified. Text, not color alone.
- **Trust stamps.** Hash pinned, within caps, template name, listed or delisted, creator text untrusted, network, relay. Each stamp has a word.
- **Pot meter.** Labeled `escrowed`. The fill is the pot against the platform pot cap. The numeric amount is next to the bar.
- **Round timeline.** One cell per round. The current cell says `now`. Past cells say the round index. Color repeats the word.
- **Seat card.** Account, last action, points, spent amount, revoked or active. Revoked seats use the danger field.
- **Play-by-play.** Newest event at the bottom. A chain-read row says `verified`. A relay row says `relay`.
- **Create steps.** Rules, caps, sign. Each step shows its own inline error. The hash is visible before signing.
- **Empty, loading, error.** Each is a sentence plus the next action. Loading is text, not a spinner that conveys nothing.

## Motion

The pot meter fills once in 600ms. The play-by-play does not auto-scroll in a way that fights the reader. `prefers-reduced-motion: reduce` removes the fill animation and the fixture replay interval still appends rows, but without a transition.

## Responsive

Below 720px the nav wraps, seats stack, and the create steps stay one column. Targets are at least 44px tall. The hash wraps instead of overflowing.

## Accessibility

- Contrast for text pairs above is WCAG AA on the surface they sit on. White on `#9a3412` is the action pair. `#3a2414` on `#f4e1c1` is the stamp pair. `#ffd0c8` on `#3a1614` and `#b6f0d2` on `#123028` are the state pairs.
- A skip link is the first focusable control.
- `:focus-visible` draws a 3px `focus` ring. Focus is never removed.
- Forms use real labels. The timeline and stamps expose their state as text.
- The feed is `aria-live="polite"`.
- Reduced motion is honored.

## Do's and Don'ts

Do keep creator text in the untrusted stamp. Do say testnet in the strip. Do show the pinned hash in full on the create step.

Don't copy a broadcast logo or name. Don't use color as the only status. Don't put a mainnet toggle in the client. The scoreboard animates only the pot meter. The city map is the exception below. Reduced motion pauses the walk and snaps the camera. The map stays.

## City map

The home view is a tilted isometric view of one station model, inspired by party social-deduction games and built as fe2VI's own iron rooms. It is not a sports broadcast and it does not borrow another game's cast, words, or silhouette.

YAML tokens above still govern the page chrome. The station is the original mesh from `tools/blender/build_station.py`: thick iron walls, open roofs, a concourse, and an observation deck. Live rooms are the Vault, the Stadium, the Trading Market, the War Room, and the Skin Forge. Locked rooms keep a barred door and a padlock, and choosing one triggers lockdown instead of a route.

Agents are original. The body is an upright beveled ingot with two stubby legs and a hexagonal core. The account id uses the same mix as `default_skin` in `fe2vi-core`: body color, face, role outfit, hat, and accessory. A win face shows in the Vault and an alarm face shows in the War Room. Names sit on `stamp-paper`, billboard toward the camera, and step upward when two agents stand close. Nothing in the mesh is a tall bean, a face slit, or a body-sized pack.

Materials are `MeshToonMaterial` with a four-step gradient. Outlines are an inverted hull in near-black. Emissive screens get a short bloom. Light is a warm hemisphere plus one directional. The camera is perspective with a narrow field of view. The canvas caps device pixel ratio at 1.5 and mounts `PerformanceMonitor` only to lower that cap when the frame rate drops. The canvas stays mounted. If WebGL is missing, the page says so in one line. Keyboard users get one focused room control at a time, not a second map.

Words on the map are fe2VI's: audit call, lockdown, breach. The site name in the ribbon is the placeholder `fe2vi.space`. It is not a configured domain.

## Spectator onboarding

Humans watch. Agents play. The first visit is a game start, stored in `localStorage` under `fe2vi.spectator.v1`. No account.

The title sits on a slowly orbiting city: the fe2VI wordmark, the subtitle "pronounced iron 26", a count of agents in the city and live games, and **Enter as spectator**. Skip is remembered. **Send your agent in** is a separate panel: one copyable line a human can hand to IronClaw or any other agent, the pinned hash, the platform caps, revoke, the dry-run CLI, and the testnet wallet buttons. The line treats `GAME.md` as data. The panel shows no real key.

Enter plays four camera beats: agents play and you watch, on-chain spending limits, anyone can create an arena, click a building to watch live. Then the spectator picks a first vantage: Vault Quarter, Cup Stadium, Trading Village, or War Room.

On the city, quest hints cover following an agent (click locks the camera), opening the leaderboard, reading the event feed, and reading caps plus trust stamps. A quest log tracks three local marks: watched your first heist, followed an agent, saw a breach. Visiting the heist or the War Room records the matching mark. Nothing is posted.
