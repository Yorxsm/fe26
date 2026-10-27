# Agent scale

Measured on the production build, headless Chrome, `--use-gl=angle --use-angle=swiftshader`, viewport 1280×800, device pixel ratio 1.00. The page had already entered as a spectator. The post stack stayed on: SSAO, depth of field, bloom, and ACES tone mapping. The FPS counter reads `renderer.info.render.calls` after the composer pass and writes `#stress-readout` every 0.5s of frame time. Swiftshader plus that post stack is the whole frame budget. These numbers are the runs, not a target.

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

- The clip is `idle` when paused, in a lineup, on a poster, or when speed is below 0.4. Otherwise it is `celebrate` near the vault, `caught` near the war room (nearest district within 3.4), or `walk`.
- A clip change calls `stopAllAction`, then `clipAction(clip).reset().play()`. There is no crossfade and no blend.
- Walk `timeScale` is `clamp(speed / 1.7, 0.25, 3.2)`. Other clips stay at 1.
- The face is a canvas texture redrawn every frame (blink, saccade, look). Each full agent also has a contact-shadow disc, a trail of 18 particles, an HTML name tag, and, at normal crowd size, a rim point light.

## Stress mode

`?agents=N` (capped at 5000) mounts a crowd and the FPS overlay. `?agents=N&detail=all` mounts N full skinned agents. Without `detail=all`:

- Indices `0 .. 23` stay full skinned meshes (`FULL_CAP = 24`). The cap is by index, not by distance to the camera. Phases spread those 24 along the loop, so they are not a camera-near set.
- Everyone else is one `InstancedMesh` of capsules (`radius 0.16`, `length 0.62`, scaled by `AGENT_SCALE`), colored from the paint list.
- An instance is scaled to 0 when it is farther than 140, or when it is outside the frustum (NDC margin 1.15) and farther than 18. The instanced mesh itself is not frustum-culled, because culling is per instance.
- Rim lights are omitted once the count is above 24.
- The CPU still runs `placeAgent` and writes a matrix for every impostor every frame. Instancing removes draw calls, not that loop.

## Measured

Last readout after 22 updates. Wall time is load plus about 22–28s of polling. Draw calls are that frame's composer total.

| Mode | FPS | Draws | Notes |
| --- | --- | --- | --- |
| 100, LOD | 0.6 | 2546 | 22 samples, 30s wall, dpr 1.00 |
| 500, LOD | 0.5 | 2582 | 22 samples, 31s wall, dpr 1.00 |
| 2000, LOD | 0.4 | 2606 | 22 samples, 41s wall, dpr 1.00 |
| 100, `detail=all` | 0.3 | 7809 | 8 samples over 45s wall. First sample was 1.1 fps / 7773 draws while shaders settled. The next seven were 0.3 fps, draws 7695–7843. |

Draw calls stay flat from 100 to 2000 once the cap is on (2546, 2582, 2606). One hundred full agents land near 7800 draws, about 70 extra draws per full agent past the cap: `(7809 - 2546) / (100 - 24) ≈ 69`. Sub-1 fps is the software GL post stack. The same 2000-agent view on this build read 0.4 fps and 2606 draws in the overlay. A real GPU will not reproduce 0.4 fps. It will still pay those draw calls, the per-agent mixers, and the face canvases.

## Plan for 2000 concurrent agents

Shipped in this client:

- Instanced capsules for every agent past the 24 full-detail slots.
- Distance and frustum culling on those instances.
- No per-agent rim light once the crowd is over the cap.

Still to do on the client:

- Pick the 24 full-detail slots by distance to the camera, and swap them as the camera moves. The index cap does not do that.
- Replace far capsules with camera-facing impostors, then with a vertex-animation texture so the crowd animates on the GPU. The mixer, the face canvas, and the 18-particle trail stay on the full-detail set only.
- Keep frustum and distance culling, and stop writing matrices for instances that have been hidden for several frames.
- Leave the post stack on for the real lobby. It is most of the swiftshader cost. A later pass can drop SSAO resolution when the full-detail set is saturated, without turning the grade off.

Backend, not built:

- One Durable Object per arena, or per district once a district is its own crowd.
- Interest management: a socket subscribes to the district in view plus a radius, not to every agent on the map.
- The object ticks at a fixed rate and streams deltas (position, clip, mood) over the WebSocket. Clients do not apply a full snapshot each tick.
- Agents outside the interest set are not simulated on that connection. The client keeps them on the impostor path from the last delta, or omits them.
