# Agent scale

Measured on the production build, headless Chrome, `--use-gl=angle --use-angle=swiftshader`, viewport 1280×800, device pixel ratio 1.00. The page had already entered as a spectator. The post stack stayed on: SSAO, depth of field, bloom, and ACES tone mapping. The counter reads `renderer.info.render.calls` after the composer and writes `#stress-readout` every 0.5s of frame time.

Draw calls are the number that matters. Swiftshader FPS stays under 1 with this post stack, on an empty village and on a crowd. A real GPU will not reproduce that FPS. It will still pay the draws.

## What the city does today

`placeAgent` in `apps/web/lib/city.ts` is the whole movement model.

- One polyline, `WALK_LOOP`, eight xz stops around the plots.
- Phase is `index * 1.7`. Travel is `time * 0.28 + phase`, wrapped across the eight stops.
- Between stops the position is a smoothstep, `t * t * (3 - 2 * t)`.
- Speed is `distance * 0.28 * ease` with `ease = 6 * t * (1 - t)`. It peaks mid-segment and is zero at the waypoints.
- Paused agents snap to their home waypoint (`index % 8`), rotation 0, speed 0.
- There is no pathfinding, no collision, and no avoidance. Agents on the same loop occupy the same line and stack when their phases meet.
- `poseClearOf` is the only extra placement. If an interior camera is open and the agent is within 8 units of that camera, it is parked on a short offset beside the camera. That is a framing hack, not steering.

Animation is a per-agent `AnimationMixer` on a cloned skinned mesh.

- The clip is `idle` when paused, in a lineup, on a poster other than the street frame, or when speed is below 0.4. Otherwise it is `celebrate` near the vault, `caught` near the war room (nearest district within 3.4), or `walk`.
- A clip change fades the previous action out and the next one in over 0.2s. The first clip on a new mesh plays without a fade. Poster holds (`walk`, `expressions`) still freeze a single frame and do not crossfade.
- Walk `timeScale` is `clamp(speed / 1.7, 0.25, 3.2)`. Other clips stay at 1.
- The face is a canvas texture redrawn every frame (blink, saccade, look). Each full agent also has a contact-shadow disc, a trail of 18 particles, an HTML name tag, and, at normal crowd size, a rim point light.

## Static village

`mergeGeometries` groups static meshes that share a material inside each district. Buildings, fences, pads, and terrain collapse that way. A district's batch carries that district, so opening an interior hides only that building. Roofs, screens, the vault wheel, the war holo, and the beacon stay as their own meshes because they move or hide on their own. An unused `uv2` channel is dropped before the merge when the material has no aoMap and no lightMap.

Repeated props are instanced:

- Trees, bushes, rocks, and flowers were already one `InstancedMesh` per species in `Nature`.
- Street lamps are three instanced meshes (pole, head, pool light). The twelve point lights stay.
- The lit window panes are one instanced mesh.

## Post stack

`Grade` builds an `EffectComposer` with three passes. The readout prints that count.

1. `NormalPass` draws the scene. The postprocessing override turns the shadow map off for this pass.
2. `RenderPass` draws the shadow map, then the color scene.
3. `EffectPass` is one fullscreen pass: SSAO, depth of field, bloom, and ACES together.

Static geometry is therefore drawn up to three times per frame (shadow, normals, color). The 256 draws at zero agents are those three passes over the merged village, plus the sky, the grass, the lamps, and the fullscreen pass. It is not 256 separate buildings.

## Stress mode

`?agents=N` (capped at 5000, and `0` is a real empty village) mounts a crowd and the overlay. `?agents=N&detail=all` mounts N full skinned agents. Without `detail=all`:

- The 24 full-detail slots are the agents closest to the camera (`FULL_CAP = 24`). Distance is three-dimensional. The set is reconsidered every 0.3s. A slot swaps only when a new agent is at least 4 units closer than the farthest full agent, so the set does not flicker on the boundary.
- Everyone else is one `InstancedMesh` of capsules (`radius 0.16`, `length 0.62`, scaled by `AGENT_SCALE`), colored from the paint list. A full-detail index is in that mesh too, scaled to 0, so the instance buffer does not have to be rebuilt when the set changes.
- An instance is scaled to 0 when it is one of the 24, farther than 140, or outside the frustum (NDC margin 1.15) and farther than 18. The instanced mesh itself is not frustum-culled, because culling is per instance.
- Rim lights are omitted once the count is above 24.
- The CPU still runs `placeAgent` and writes a matrix for every impostor every frame. Instancing removes draw calls, not that loop.

## Measured

Same camera and post stack as the previous pass. `?agents=0` is the village with no crowd. Draw calls on 100 and 2000 move because the 24 full agents walk into and out of the color frustum. The impostor mesh does not add a draw per agent, so 2000 is not more expensive than 100 on draws. A lower 2000 sample means fewer of those 24 were in frame, not a cheaper crowd.

| Agents | Draws | Notes |
| --- | --- | --- |
| 0 | 256 | 22 samples, 31s wall, dpr 1.00. 21 samples were 256. The first was 3.5 fps while shaders settled, then 0.8–0.9 fps. |
| 100 | 1315–2588 | 24 samples, 33s wall. Median of the last 10 samples is 1438. |
| 2000 | 1090–2711 | 28 samples, 40s wall. Median of the last 10 samples is 1244. One of those frames was 2537 when a new set walked into view. |

Previous pass, before the village merge, same viewport and post stack: 2546 draws at 100, 2582 at 500, 2606 at 2000. That number barely moved because the unmerged village was most of it. One hundred full agents (`detail=all`) on that build was 7809 draws, about 70 draws per extra full agent past the cap. Sub-1 fps was, and still is, the software GL post stack.

## Plan for 2000 concurrent agents

Shipped in this client:

- Static village geometry merged by material, per district. Lamps and window panes instanced. Trees were already instanced.
- The post stack counted: 3 composer passes, and the normal pass does not redraw shadows.
- The 24 full-detail slots follow camera distance, with a 4-unit swap margin.
- Clip changes crossfade over 0.2s.
- Instanced capsules for every agent outside those 24 slots, with distance and frustum culling.
- No per-agent rim light once the crowd is over the cap.

Still to do on the client:

- Replace far capsules with camera-facing impostors, then with a vertex-animation texture so the crowd animates on the GPU. The mixer, the face canvas, and the 18-particle trail stay on the full-detail set only.
- Stop writing matrices for instances that have been hidden for several frames.
- Leave the post stack on for the real lobby. A later pass can drop SSAO resolution when the full-detail set is saturated, without turning the grade off.

Backend, not built:

- One Durable Object per arena, or per district once a district is its own crowd.
- Interest management: a socket subscribes to the district in view plus a radius, not to every agent on the map.
- The object ticks at a fixed rate and streams deltas (position, clip, mood) over the WebSocket. Clients do not apply a full snapshot each tick.
- Agents outside the interest set are not simulated on that connection. The client keeps them on the impostor path from the last delta, or omits them.
