# Assets

The station, rooms, and agents under `apps/web/public/models/` are original meshes. They are produced by `tools/blender/build_station.py` and then meshopt-compressed. They are not traced from a character pack and they are not imported from the CC0 kits below.

Rebuild from the repo root:

```sh
blender --background --python tools/blender/build_station.py
npx --yes @gltf-transform/cli@4.1.1 meshopt apps/web/public/models/station.glb apps/web/public/models/station.glb
npx --yes @gltf-transform/cli@4.1.1 meshopt apps/web/public/models/agent.glb apps/web/public/models/agent.glb
```

Blender 4.0 builds Z-up. The exporter writes glTF Y-up. `layout.json` is in glTF meters. Room markers inside `station.glb` are empties named `marker.<district-id>`.

| File | Bytes | What it is |
| --- | ---: | --- |
| `apps/web/public/models/station.glb` | 168368 | Open-roof iron station: Vault, Stadium, Trading Market, War Room, Skin Forge, locked Oracle Plaza, Launch Pier, and Card Club, plus the concourse and observation deck |
| `apps/web/public/models/agent.glb` | 53576 | Beveled ingot with stubby legs, a hexagonal core, and the catalog parts below |
| `apps/web/public/models/layout.json` |  | Room ids and glTF positions for the scene |

Both GLBs use `EXT_meshopt_compression` (plus `KHR_mesh_quantization` and `KHR_materials_emissive_strength`). Draco is not used: this Blender build has no Draco encoder, and a CDN decoder is the wrong trade. The scene loads them with three's `MeshoptDecoder`.

The agent clips are `idle`, `walk`, and `celebrate` on `bone.root`, `bone.body`, `bone.leg.L`, and `bone.leg.R`. The walk swing is bone-local. Confirm the axis when the city plays the clip.

Trait indexes match `crates/fe2vi-core/src/skin.rs`. The scene shows one face and the parts for the equipped skin. A default skin hides every `hat.*` and `acc.*` the seed did not pick. Body color is the `Body` material, not a separate mesh. Trails are particles, not geometry.

| Index | Hat node | Role outfit | Accessory nodes | Face node |
| --- | --- | --- | --- | --- |
| 0 | none | safecracker, uses hat.1 | none | `face.0` neutral core |
| 1 | `hat.1` beanie | trader, visor plus headset | `acc.1.L`, `acc.1.R`, `acc.1.band` | `face.1` |
| 2 | `hat.2` visor brim | hacker, hood plus antenna | `acc.2`, `acc.2.handle` magnifier | `face.2` alarm |
| 3 | `hat.3` hood | auditor, magnifier | `acc.3.L`, `acc.3.R`, `acc.3.strap` goggles | `face.3` |
| 4 | `hat.4` hard hat | builder, hard hat | `acc.4`, `acc.4.strap` hip kit |  |
| 5 | `hat.5`, `hat.5.tip` antenna | scout, goggles plus hip kit | `acc.5` bolt |  |

The hip kit is a side satchel. The body stays a boxy ingot.

`apps/web/components/CityMap.tsx` loads both files with drei `useGLTF` and three-stdlib's meshopt decoder. Draco stays off so the page does not fetch a decoder.

Kenney's CC0 kits stay vendored and unused as characters, so the silhouette stays the ingot.

| File | Source | License |
| --- | --- | --- |
| `apps/web/public/assets/cc0/building-a.glb` through `building-h.glb`, `detail-awning.glb` | [City Kit (Commercial)](https://kenney.nl/assets/city-kit-commercial) 2.1, Kenney | [CC0 1.0](http://creativecommons.org/publicdomain/zero/1.0/) |
| `road-straight.glb`, `road-crossroad.glb`, `construction-cone.glb` | [City Kit (Roads)](https://kenney.nl/assets/city-kit-roads) 2.2, Kenney | CC0 1.0 |
| `character-a.glb` through `character-d.glb` | [Blocky Characters](https://kenney.nl/assets/blocky-characters) 2.0, Kenney | CC0 1.0 |

Kenney asks for a credit, and it is not a license requirement. Credit: Kenney (www.kenney.nl).

No Quaternius file is in the tree.
