# fe2VI art spec v3: TECHNOLOGY VILLAGE (overrides v2 layout)

## FOUNDER DIRECTION v3 (overrides everything below that conflicts)
No more connected pods or a station floating in the void. fe2VI is an OUTDOOR TECHNOLOGY VILLAGE:
- Terrain: rolling green grass (PBR grass texture + instanced grass blades near the camera), dirt/stone paths, small hills,
  clusters of realistic trees (oak/pine/birch, instanced, wind sway), bushes, flowers, rocks, a pond or stream, lamp posts.
- Arenas are separate, BIG buildings (each ~3-4x the old pod footprint) spaced out across the village and joined by
  paved roads with kerbs, road markings, street lamps, benches, signposts with arena names, small solar panels and antennas.
- Dreamy realism: golden-hour sun, soft pastel sky with volumetric clouds, light haze/fog in the distance, god rays through trees,
  soft shadows, AO, DOF on fly-ins, ACES/AgX tone mapping, warm grade. Keep it a bit stylized but clearly realistic materials.
- Each arena is a well-decorated building, exterior AND interior (camera flies inside on click; walls/roof fade or cut away):
  - Vault Bank (Heist Night): stone/steel bank facade with columns, a gold sign, inside a marble floor, huge round vault door,
    gold bar pyramids and coin heaps, velvet ropes, cameras, laser tripwires during heists, chandelier.
  - Trading Cup Stadium: open-air mini stadium with stands, flags, floodlights, a giant scoreboard.
  - 24/7 Trading Market: covered market hall/plaza with striped-awning stalls, string lights, crates, a giant leaderboard screen with a live chart.
  - Hacker War Room: modern glass-and-concrete tech HQ with a rooftop dish; inside desks, multi-monitor terminals,
    world-map wall, holo table, red alarm beacon during breaches, cables, plants, couches.
  - Skin Forge: brick workshop with a chimney and smoke, glowing furnace, anvil, tool racks, a display pedestal.
  - Coming-soon plots: fenced construction sites with scaffolding, cones, a crane and a "COMING SOON" billboard.
- Interiors must be decorated like a real lived-in place: rugs, shelves, posters (our own IP), plants, lamps, furniture, wall art, clutter.
- Agents (6 roles from the character sheet) walk along the roads between buildings, enter and exit doors.
- Overview camera: high 3/4 view showing the whole village; click a building to fly to it and then inside.
- Use CC0 assets (Poly Haven / ambientCG / Kenney / Quaternius) or Blender-built models; credit each in docs/ASSETS.md.
  Keep performance sane: instancing, LODs, KTX2/meshopt, under ~25 MB total assets, 60fps target on a mid laptop.

# (older v2 notes; materials/agent/font notes still apply)

## FOUNDER DIRECTION (overrides anything below that conflicts)
The previous build "looks like lazy art". The founder wants it to look a bit REALISTIC and DREAMY, NOT on a black canvas.
- No black void. The station floats above a soft dreamy sky: golden-hour / pastel gradient (peach #ffcf9e -> lilac #b9a3e8
  -> soft blue #8fb8ff), volumetric sunbeams, drifting cloud sea below (layered sprite/volumetric clouds, slow parallax),
  distant floating islands/towers silhouettes in atmospheric haze. HDRI environment lighting (use a CC0 Poly Haven HDRI,
  credit in ASSETS.md) for reflections.
- Rendering: switch from flat toon to stylized-realistic PBR (MeshStandard/Physical): brushed metal with roughness/normal
  maps, real gold with high metalness, glass screens with emissive + reflections, wood grain on stalls, cloth awnings.
  Use CC0 textures (Poly Haven / ambientCG) committed as compressed KTX2/webp, credited. Drop the inverted-hull outlines
  (or make them very subtle). Soft shadows (PCSS or baked AO/lightmaps from Blender), SSAO, gentle bloom, depth of field
  on fly-ins, light fog, film-grain-free filmic tone mapping (ACES/AgX), slight warm colour grade.
- Agents: same silhouettes/roles as the character sheet but rendered like premium collectible vinyl/metal toys:
  PBR painted metal, glossy glowing face screen, soft subsurface-ish highlights, contact shadows.
- Detail density: every room must feel hand-dressed (props, cables, decals, wear, small lights). No empty grey boxes.
- The "dark steel / black background" colour notes below now apply only to the station's metal; the world around it is bright and airy.


Target: the screenshots must read as the same game as docs/concept/city-concept.jpg and agent-sheet.jpg.
Rules: push to main after every numbered step (at most ~20 min between pushes), gitleaks before each push,
original IP only, 3D only, testnet only, no deploys. Commit the spec as docs/ART_SPEC.md first.

## 0. Global look
- Camera: isometric-ish 3/4 top-down view of the WHOLE station (like the concept), orthographic or a narrow FOV (~25deg) perspective,
  pitch ~50deg, yaw ~45deg. Click a room to fly in; ESC flies back out.
- Full-screen canvas (100vw x 100vh). No page layout, no scrolling page sections. Lobby list, leaderboards, game
  lists all become in-game HUD panels (dark slate #14181d at 90% opacity, 2px border #3a3f46, 6px corner bevel, orange
  title tab #d8692a with cream text #f3e6c8, chunky rounded display font).
- Floor: dark steel #2a2e33 plates in a 2m grid with lighter seams #3b4046, rivets at corners, subtle orange hazard
  strips at doorways. Background: dreamy sky + cloud sea (see founder direction), never black.
- Walls: thick (0.6m) octagonal rooms, wall height ~2.4m, cut open at the camera-facing side (cutaway) so the inside is visible.
  Panels with bevels, vertical pipes, small amber wall lights (#ffb347, emissive) every ~3m, orange glowing vents near the floor.
- Corridors: 3m wide, metal grate floor, connecting all rooms as in the concept layout (vault left, stadium top centre,
  market right, war room bottom centre, two locked rooms top-right and bottom-left).
- Lighting: low ambient (0.25), each room has its own warm/coloured key light + emissive props; bloom only on emissives.
  Toon ramp with 3 bands + black inverted-hull outlines (2-3px look).
- Wooden-sign room labels: dark plate, cream caps text, floating over each room ("VAULT ROOM", "STADIUM ROOM",
  "TRADING MARKET 24/7 OPEN", "HACKER WAR ROOM", "COMING SOON").

## 1. Vault Room (Heist Night) - gold/amber
- Huge round vault door (diameter ~3m) on the back wall: concentric rings, 12 radial bolts, a 3-spoke wheel handle that
  slowly rotates during heists.
- Two big piles of gold: stacked ingot pyramids (~40 bars, beveled boxes, #f2b227 with #fff1a8 highlight) + coin heaps
  (instanced cylinders). Gold is emissive-ish at 0.3 so it glows under bloom.
- Small orange crate by the door, wall-mounted cameras with red LEDs, laser tripwire beams (red, animated) when a heist is live.
- Light: warm amber key from above the gold.

## 2. Stadium Room (Trading Cup) - teal/green
- Tiered bleachers (4-5 steps) on both sides, octagonal arena floor with an orange ring + centre emblem decal.
- Big wall scoreboard: "TRADE LEAGUE / SEASON N" with team rows (team icon dot, name, score) in teal #3ee0b5 on dark screen,
  rendered from live/mock data as a canvas texture.
- Spotlights pointing at the centre.

## 3. Trading Market (24/7 Village) - neon multi-colour
- 6-8 market stalls: wooden counter + striped awning (red/white, teal, purple, green), small crates/goods on top, a lantern.
- Giant "MARKET LEADERBOARD" screen: rank, player, net worth + a green line chart (live canvas texture).
- Small cart icon sign "TRADING MARKET 24/7 OPEN" in green.
- Barrels, crates, potted lamps scattered. Ambient chatter sound.

## 4. Hacker War Room (CTF) - cyan/red
- Ring of desks with multi-monitor terminals showing scrolling cyan text (canvas texture), a big world-map screen on the back wall.
- Central round holo table with a rotating cyan hologram (globe or network graph).
- Red rotating alarm beacon on the ceiling that spins and flashes red during a "breach".
- Light: cool cyan with red pulses.

## 5. Skin Forge - orange/molten
- Anvil, forge furnace with glowing molten orange (#ff6a1a) mouth, hanging tools, a pedestal where the previewed agent stands.

## 6. Coming-soon rooms
- Smaller octagonal rooms, dim, a large padlock icon in the middle, "COMING SOON" sign + "NEW ROOM IN DEVELOPMENT" plaque, small orange lights.

## 7. Agents (match agent-sheet.jpg)
- Body: chunky rounded-rectangle "ingot" torso ~0.9m tall, short stubby legs and arms, big octagonal face screen taking
  ~55% of the front, two simple black oval eyes (blink every 3-6s), screen is emissive in the role colour.
- Small antenna with a ball on top (most roles). Soft bevels everywhere, panel lines, little bolts.
- Roles (body colour / screen colour / accessory):
  - Safecracker: grey #9aa0a6 / yellow #ffc928 / black beanie with keyhole patch
  - Trader: purple #7a4fc4 / magenta #e04bff / headset with mic + gold trim
  - Hacker: navy #1d2a3a / cyan #2fe6e6 / dark hoodie with hood up, angry brow eyes
  - Auditor: white #e7e4dc / yellow / magnifying glass in hand + gold badge
  - Builder: orange #e0742a / orange / hard hat + tool-badge chest plate
  - Scout: olive #6b7a3a / lime #9cff3a / goggles on head + backpack
- Floating name tag above each agent: white rounded pill, black text, small pointer (as in concept), always facing camera, no overlap.
- Animations: idle bob + blink, walk (confirm bone axis, feet must not slide), celebrate jump with arms up, "caught" slump.
- Agents wander between rooms along corridors on paths; 8-12 visible at once in mock mode with names like the concept
  (use our own names).

## 8. Intro / onboarding
- Same 3D world, cinematic: camera dolly over the station, each beat flies to a real room with its real props
  (vault door turning, market board ticking, war-room alarm, cap-holds moment). Letterbox, big serif-free display captions,
  skip button. Sound synced.

## 9. Acceptance per push
- Take the same-angle screenshot as the concept (overview) + one per room + agent line-up (all 6 roles side by side,
  same framing as agent-sheet.jpg). Put concept vs build side by side in one compare-*.png artifact and list remaining gaps.
- Performance: keep 60fps on a mid laptop; instance props; texture atlases; GLBs meshopt-compressed.

## Push order
1 spec+concept files+favicon/OG  2 full-screen + HUD + camera  3 station shell (floors/walls/corridors/lights)
4 vault  5 market  6 stadium  7 war room  8 forge + coming soon  9 agents (all 6 roles)  10 intro rework  11 compare shots

## Build check (v3 village)

The village is Kenney shells on the pads, under the Poly Haven dawn HDRI, with a corner camera inside each live room. Plates: `overview.png`, `exteriors.png`, `interiors.png`, `lineup.png`, `intro.png`, `compare-overview.png`, `compare-roles.png`.

What is in the scene:

- Dawn HDRI is the sky and the light. The cloud planes that streaked the upper left of the overview are gone. There are no god-ray planes.
- Trees are the smoother Kenney round and detailed variants, smooth-shaded, instanced, with a wind sway. They stay off the roads and the building fronts, and trees between the camera and its target are culled. They are still a few hundred vertices each, not scanned trees.
- Buildings use the Kenney commercial atlas with the lilac wall texels replaced: vault sandstone, market timber, war room dark glass, forge red brick, stadium and the plots a warm stone. Awnings, a portico, a gold VAULT board, string lights, stands, floodlights, flags, a dish, a chimney, smoke, fences, cones, scaffold, and a gantry crane are in place. COMING SOON is 3D text on the plots.
- Interiors are lazy-loaded Kenney furniture rooms. The camera sits at eye height in the open corner. The wall on the camera side is hidden, and agents are parked off that spot.
- Six original ingot bodies share one rig (idle, walk, celebrate): grey beanie with a keyhole, purple headset and gold trim, navy hoodie, cream coat with a magnifier and badge, orange hard hat, olive goggles and a hip pack. `?frame=lineup` stands them in a row. The city shows those bodies, cycling the six roles.

Still short of the spec:

- The kits are not designed buildings. The war room is a shortened tower plus a dish. The vault columns are a portico. The forge chimney is a cabin chimney. The crane is a gantry, not a tower crane.
- Rooms are three walls and a floor. The stadium interior is a locker at the edge of the field, not the bowl. One side of each room is sparser than the other.
- Agents are block ingots with role props, not sculpted characters. The walk cycle can slide the feet. The scout pack is a hip kit on the ingot.
- `station.glb` still holds the old procedural village. Those meshes stay hidden except the plaza pads. Models and textures together are about 12 MB. There are no LODs. Frame rate was not measured on a laptop GPU. These shots are software WebGL.

Earned rare traits and the 5% creator royalty live on the skin token. The table and the bind rules are in `docs/SKINS.md`. They are paint. They do not change scoring or caps.
