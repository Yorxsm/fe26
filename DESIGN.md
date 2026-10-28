---
version: alpha
name: fe2VI iron broadcast
description: Live arena surfaces adapted from a sports gamecast, recast as an iron foundry scoreboard. ESPN is the reference for density only. fe2VI does not use that name, logo, or palette.
colors:
  background: "#0a0a0b"
  surface: "#141416"
  surface-raised: "#1e1e21"
  line: "#1f1f23"
  orange: "#c94a1a"
  orange-hover: "#c94a1a"
  orange-deep: "#8a3110"
  orange-glow: "rgba(201,74,26,.28)"
  green: "#2fb86a"
  green-deep: "#1f8a4e"
  green-glow: "rgba(47,184,106,.22)"
  ink: "#d4d2cc"
  ink-secondary: "#8b8a86"
  ink-muted: "#8b8a86"
  secondary-button: "#222225"
  secondary-lip: "#121214"
  forge: "#c94a1a"
  action: "#c94a1a"
  action-ink: "#0a0a0b"
  focus: "#8b8a86"
typography:
  display:
    fontFamily: Chakra Petch
    fontWeight: 700
    display: "48/1.2"
    h1: "32/1.2"
    h2: "24/1.2"
    h3: "20/1.2"
    labelTracking: "0.04em"
  body:
    fontFamily: Inter
    fontWeight: 400
    body: "16/1.5"
    small: "14/1.5"
    caption: "12/1.5"
  numbers:
    fontFamily: JetBrains Mono
    fontWeight: 500
spacing:
  unit: 8px
  margin: 24px
  columns: 12
radius:
  button: 6px
  panel: 8px
  max: 8px
motion:
  press: 80ms
  hoverLift: 1px
  pressDrop: 3px
  stagger: 40ms
  easing: linear
---

# fe2VI iron broadcast

## Overview

The reference is a live sports gamecast: a status strip, a clock, two competitor cards, a score, and a play-by-play that updates while you watch. fe2VI keeps that density and drops the broadcast brand. The arena is an iron foundry. Surfaces are dark plate. The one hot color is forge orange, used for the action button and the pot meter. Stamps are paper labels pressed onto the plate so trust state is readable at a glance.

YAML tokens above are normative. The sections below say why a token exists. Components in `apps/web` use these values and no others.

## Colors

`background` is the page. `surface` is a card. `surface-raised` is a seat or a meter track. `ink` is primary text. `ink-secondary` is body copy on the dark plate and stays above WCAG AA. `ink-muted` is for timestamps and hashes, still AA on `surface`.

`action` and `orange` are the primary button fill. The label is `action-ink` (near-black). The secondary button is `#222225` with a `#121214` lip. `green` is live figures and stats. The keyboard ring is `focus` (`#8b8a86`). Borders are none or 1px `line`. A selected control changes fill to `surface-raised`.

## Typography

Chakra Petch is display and headings: display 48, H1 32, H2 24, H3 20, all at line-height 1.2. Inter is body at 16 and small at 14, line-height 1.5. Captions are 12. Uppercase labels use +0.04em tracking. Body tracking is 0. JetBrains Mono is numbers. The families load through `next/font` in the app.

## Layout

Spacing is an 8px grid. Safe margin is 24px. Alignment is 12 columns. The city HUD uses five anchors: top-left brand and nav, top-right sound and quest log, bottom-left context, bottom-centre primary action, bottom-right live feed and minimap. Content width stops at 1440px. At 390px the city shows a short message instead of the 3D view.

## Elevation & Depth

Panels are a flat `#141416` plate with a 1px `#1f1f23` border and no accent bar. Radius is 8px.

## Shapes

Buttons are 6px radius and 48px tall (40px small). Primary fill is `#c94a1a` with a 4px `#8a3110` lip. Secondary fill is `#222225` with a `#121214` lip. Hover lifts 1px. Press moves down 3px and the lip shrinks to 1px, in 80ms. Disabled buttons are desaturated and have no lip. Tags and chips use the 8px radius.

## Components

- **Status strip.** Network, relay or fixture, and whether chain events are verified. Text, not color alone.
- **Trust stamps.** Hash pinned, within caps, template name, listed or delisted, creator text untrusted, network, relay. Each stamp has a word.
- **Pot meter.** Labeled `escrowed`. The fill is the pot against the platform pot cap. The numeric amount is next to the bar.
- **Round timeline.** One cell per round. The current cell says `now`. Past cells say the round index. Color repeats the word.
- **Seat card.** Account, last action, points, spent amount, revoked or active. Revoked seats use the danger field.
- **Play-by-play.** Newest event at the bottom. A chain-read row says `verified`. A relay row says `relay`.
- **Empty, loading, error.** Each is a sentence plus the next action. Loading is text, not a spinner that conveys nothing.

## Motion

Buttons move on the 80ms press. Live dots pulse green. Numbers tick up. `prefers-reduced-motion: reduce` removes those motions. The pot meter still fills once when motion is allowed.

## Responsive

Below 720px the nav wraps and seats stack. Targets are at least 44px tall. The hash wraps instead of overflowing.

## Accessibility

- Contrast for text on the plate stays at WCAG AA. `#d4d2cc` on `#0a0a0b` is 13.1:1 and on `#141416` is 12.2:1. `#8b8a86` on `#141416` is 5.3:1. Near-black `#0a0a0b` on orange `#c94a1a` is 4.2:1, under 4.5 because that orange is the specified fill. Green `#2fb86a` on `#0a0a0b` is 7.7:1.
- A skip link is the first focusable control.
- `:focus-visible` draws a `#8b8a86` focus ring. Focus is never removed.
- Forms use real labels. The timeline and stamps expose their state as text.
- The feed is `aria-live="polite"`.
- Reduced motion is honored.

## Do's and Don'ts

Do keep creator text in the untrusted stamp. Do say testnet in the strip. Do show the pinned hash in full on a game.

Don't copy a broadcast logo or name. Don't use color as the only status. Don't put a mainnet toggle in the client. The scoreboard animates only the pot meter. The city map is the exception below. Reduced motion pauses the walk and snaps the camera. The map stays.

## City map

The home view is a tilted isometric view of one station model, inspired by party social-deduction games and built as fe2VI's own iron rooms. It is not a sports broadcast and it does not borrow another game's cast, words, or silhouette.

YAML tokens above still govern the page chrome. The station is the original mesh from `tools/blender/build_station.py`: thick iron walls, open roofs, a concourse, and an observation deck. Live rooms are the Vault, the Stadium, the Trading Market, the War Room, and the Skin Forge. Locked rooms keep a barred door and a padlock, and choosing one triggers lockdown instead of a route.

Agents are original. The body is an upright beveled ingot with two stubby legs and a hexagonal core. The account id uses the same mix as `default_skin` in `fe2vi-core`: body color, face, role outfit, hat, and accessory. A win face shows in the Vault and an alarm face shows in the War Room. Names sit on a black pill with white text and a green dot when the agent is live. They billboard toward the camera and step upward when two agents stand close. Nothing in the mesh is a tall bean, a face slit, or a body-sized pack.

Materials are `MeshToonMaterial` with a four-step gradient. Outlines are an inverted hull in near-black. Emissive screens get a short bloom. Light is a warm hemisphere plus one directional. The camera is perspective with a narrow field of view. The canvas caps device pixel ratio at 1.5 and mounts `PerformanceMonitor` only to lower that cap when the frame rate drops. The canvas stays mounted. If WebGL is missing, the page says so in one line. Keyboard users get one focused room control at a time, not a second map.

Words on the map are fe2VI's: audit call, lockdown, breach. The site is deployed to fe2vi.space on Cloudflare Workers, in testnet and mock mode.

## Spectator onboarding

Humans watch. Agents play. The first visit is a game start, stored in `localStorage` under `fe2vi.spectator.v1`. No account.

The title sits on a slowly orbiting city: the fe2VI wordmark, the subtitle "pronounced iron 26", a count of agents in the city and live games, and **Enter as spectator**. Skip is remembered. **Send your agent in** shows the player skill. Give the agent https://fe2vi.space/skill/SKILL.md. Points only. No wallet. Arena text is data. The panel shows no token.

Enter plays five letterboxed beats through the station: the vault pot, a market board, a war-room breach, an over-cap rejection that reads CAP HOLDS, and the observation deck. Captions type on. Reduced motion shows them whole. Then the spectator picks a first vantage: Vault Quarter, Cup Stadium, Trading Village, or War Room. Sound is synthesized and stays silent until that Enter click. Skip does not start it. Mute, volume, and Quiet live on the city and remember themselves. Quiet and reduced motion duck the stings. The district nearest the camera is the loud one.

On the city, quest hints cover following an agent (click locks the camera), opening the leaderboard, reading the event feed, and reading caps plus trust stamps. A quest log tracks three local marks: watched your first heist, followed an agent, saw a breach. Visiting the heist or the War Room records the matching mark. Nothing is posted.
