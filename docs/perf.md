# Performance

Measured in headless Chrome with `--use-gl=angle --use-angle=swiftshader` against `next start`. The network throttle is Chrome's Fast 4G preset: 1.6 Mbps down, 750 Kbps up, 150 ms latency. Cache was disabled. `renderer.info` is read from a probe in the canvas. The postprocessing composer does not add its passes to that counter, so draw calls and triangles there stay near 1 / 12 even while the village is on screen. Texture and geometry counts from `renderer.info.memory` do move.

## Before

The loader waited on the whole village.

| Asset | On disk |
| --- | --- |
| `station.glb` | 3,930 KB |
| `kits.glb` | 1,679 KB |
| `agent.glb` | 499 KB |
| props, nature, and the HDR | about 2.4 MB more, preloaded with the city |

On Fast 4G the loader had not finished after 60 seconds. The model responses had barely started. Draw calls were not available because the scene never became playable in that run.

## After

`npm run optimize-models` reads `apps/web/models-src` and writes the files below. The core keeps terrain, roads, and pads. Trees, the duplicate building shells, and the pier, club, and oracle scaffolds are pruned. Textures are WebP, 1024 on desktop and 512 on a narrow screen. Geometry is `EXT_meshopt_compression`. `MeshoptDecoder` is the copy bundled with Three.js. Kit atlases stay as authored, because recompressing them dropped the alpha mask and the buildings disappeared. They are still deduped, pruned, and meshopt-compressed.

| Asset | On disk | Role |
| --- | --- | --- |
| `station-core.glb` | 455 KB | Desktop ground. The loader waits for this. |
| `station-core-512.glb` | 200 KB | The same ground for a viewport under 800 px. |
| `kits.glb` | 1,269 KB | Building kits. Requested after the loader finishes. Was 1,679 KB. |

Fast 4G, desktop core, same browser:

| | |
| --- | --- |
| `station-core.glb` transferSize | 359 KB |
| Time to download that file | 1.9 s |
| Time until the loading screen cleared | 11.8 s |
| `renderer.info.memory` after the vault opened | 10 textures, 113 geometries |
| `renderer.info.render` | 1 call, 12 triangles. The composer draws the real scene outside this counter. |

The ground file is under the 1.5 MB village budget, and that file alone arrives in under 3 seconds on this throttle. The loading screen does not. At 150 ms round trip the script chunks sit in front of the model, and the full page took 11.8 seconds. Kits, props, nature, and the HDR start only after that screen clears, so they are not in the 11.8 seconds.

SwiftShader did keep the context for these runs. The village screenshot shows the compressed ground, paths, and building volumes. The vault screenshot shows the in-room board and the activity panel. The entry wall is still dark in this software GL view.
