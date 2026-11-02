# Performance

Measured on 2 Nov 2026 from production `next build` output. The before build is commit `6e99cf4` (the city before this pass). The after build is the tree that lazy-loads interiors, caps the pixel ratio, and stops the frame loop while the city is paused.

## Before

| | |
| --- | --- |
| `/` first load, as Next reports it | 307 KB |
| All static JS, gzip | 728.5 KB |
| Largest JS chunk, gzip | 166.6 KB |
| Interior glbs preloaded with the city | vault 211 KB, market 189 KB, war 215 KB, stadium 54 KB, forge 83 KB (752 KB) |
| City models that still download up front | station 3.8 MB, kits 1.6 MB, agent 499 KB |

Draw calls, triangles, texture memory, time to interactive, and frames per second were not measured. A headless Chrome using SwiftShader lost the WebGL context once the full city started, so those counters never became available.

## After

| | |
| --- | --- |
| `/` first load, as Next reports it | 308 KB |
| `/` first-load chunks, gzip | 113.3 KB (shared chunks plus the home page chunk) |
| All static JS, gzip | 732.9 KB |
| Largest JS chunk, gzip | 166.6 KB (the Three.js runtime) |
| Interiors | not requested until a building opens. One room's glb, not all five |

The initial JavaScript is under the 300 KB gzip target. The city is not under a 3 second first playable load on a fast link, because `station.glb` and `kits.glb` are still about 5.5 MB before a frame can show the village.

## What changed

- Room interiors are a separate client chunk and are fetched when a building opens, not during the first city load.
- Device pixel ratio is capped at 1.5, and at 1 on a narrow screen or a machine with four cores or fewer.
- The canvas uses `frameloop="demand"` while the city is paused and no room is open.
- `preserveDrawingBuffer` is off.
- Interior meshes are frustum-culled.
- Mobile and other low tiers already skip shadows.
- Repeated props were already merged or instanced. That stayed.

## Not done

Textures and glbs were not re-encoded to KTX2 or Draco. That rewrite needs an encoder pass and a visual check on a GPU, and this environment could not hold a SwiftShader frame of the full city. `station.glb` (3.8 MB) is the transfer to cut next.
