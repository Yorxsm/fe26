"use client";

import { Billboard, Html, OrbitControls, PerformanceMonitor, Text, useGLTF } from "@react-three/drei";
import { DISPLAY_FONT } from "@/lib/fonts";
import { Canvas, useFrame, useThree, type ThreeEvent } from "@react-three/fiber";
import {
  BloomEffect,
  DepthOfFieldEffect,
  EffectComposer,
  EffectPass,
  NormalPass,
  RenderPass,
  SSAOEffect,
  ToneMappingEffect,
  ToneMappingMode,
} from "postprocessing";
import { World } from "@/components/World";
import { useLayoutEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import { SkeletonUtils } from "three-stdlib";
import { audio } from "@/lib/audio";
import { CITY_AGENTS, DISTRICTS, placeAgent, type CityAgent, type District } from "@/lib/city";
import { BODY_COLORS, optionalPart, visibleParts, type Mood } from "@/lib/skin";

useGLTF.preload("/models/station.glb", false, true);
useGLTF.preload("/models/agent.glb", false, true);

const ROOM_PREFIX: Record<string, string> = {
  vault: "vault-quarter",
  stadium: "cup-stadium",
  market: "trading-village",
  war: "war-room",
  forge: "skin-forge",
  oracle: "oracle-plaza",
  pier: "launch-pier",
  club: "card-club",
};

const OVERVIEW_PITCH = (50 * Math.PI) / 180;
const OVERVIEW_YAW = Math.PI / 4;
const OVERVIEW_DIST = 118;
const overviewEye = new THREE.Vector3(
  Math.sin(OVERVIEW_YAW) * Math.cos(OVERVIEW_PITCH) * OVERVIEW_DIST,
  Math.sin(OVERVIEW_PITCH) * OVERVIEW_DIST,
  Math.cos(OVERVIEW_YAW) * Math.cos(OVERVIEW_PITCH) * OVERVIEW_DIST,
);
const overviewAim = new THREE.Vector3(0, 1.2, 0);

const BEATS: Array<{ position: [number, number, number]; target: [number, number, number] }> = [
  { position: [-6, 5.2, 14], target: [-12, 1.2, 8] },
  { position: [18, 5.4, -2], target: [12, 1.2, -8] },
  { position: [14, 5.2, 18], target: [8, 1.2, 12] },
  { position: [7, 8, 9], target: [0, 1.2, 0] },
  { position: [-2, 7, -18], target: [-8, 1.3, -12] },
];

type Rig = { target: THREE.Vector3; update: () => void; autoRotate: boolean; enabled: boolean };

const aim = new THREE.Vector3();
const eye = new THREE.Vector3();

function dress(root: THREE.Object3D, recolor?: string) {
  const meshes: THREE.Mesh[] = [];
  root.traverse((node) => {
    const mesh = node as THREE.Mesh;
    if (mesh.isMesh) meshes.push(mesh);
  });
  meshes.forEach((mesh) => {
    const sources = (Array.isArray(mesh.material) ? mesh.material : [mesh.material]) as THREE.MeshStandardMaterial[];
    if (recolor) {
      sources.forEach((material) => {
        if (material.name === "Body" && material.color) material.color.set(recolor);
      });
    }
    sources.forEach((material) => {
      if ("envMapIntensity" in material) material.envMapIntensity = 0.35;
    });
    const prefix = mesh.name.split(".")[0];
    const district = ROOM_PREFIX[prefix];
    if (district) mesh.userData.district = district;
    mesh.castShadow = true;
    mesh.receiveShadow = true;
    mesh.frustumCulled = false;
  });
  return root;
}

function moodFor(x: number, z: number): Mood {
  let bestId = "";
  let bestDistance = Infinity;
  for (const district of DISTRICTS) {
    const dx = district.x - x;
    const dz = district.z - z;
    const distance = dx * dx + dz * dz;
    if (distance < bestDistance) {
      bestId = district.id;
      bestDistance = distance;
    }
  }
  if (bestDistance > 3.4 * 3.4) return "trait";
  if (bestId === "war-room") return "alarm";
  if (bestId === "vault-quarter") return "happy";
  return "trait";
}

function applyMood(root: THREE.Object3D, agent: CityAgent, mood: Mood) {
  const shown = visibleParts(agent.skin, mood);
  root.traverse((node) => {
    if (optionalPart(node.name)) node.visible = shown.has(node.name);
  });
}

function Station({ onPick }: { onPick: (district: District) => void }) {
  const gltf = useGLTF("/models/station.glb", false, true);
  const scene = useMemo(() => dress(gltf.scene.clone(true)), [gltf.scene]);
  return (
    <group>
      <primitive
        object={scene}
        onClick={(event: ThreeEvent<MouseEvent>) => {
          event.stopPropagation();
          let node: THREE.Object3D | null = event.object;
          while (node) {
            const id = node.userData.district as string | undefined;
            if (id) {
              const district = DISTRICTS.find((entry) => entry.id === id);
              if (district) onPick(district);
              return;
            }
            node = node.parent;
          }
        }}
      />
      {DISTRICTS.map((district) => (
        <Billboard key={district.id} position={[district.x, 3.15, district.z]}>
          <mesh position={[0, 0, -0.05]}>
            <planeGeometry args={[Math.max(1.8, district.station.length * 0.2), 0.46]} />
            <meshStandardMaterial color="#2a241c" roughness={0.8} />
          </mesh>
          <Text
            font={DISPLAY_FONT}
            fontSize={0.18}
            color="#f3e6c8"
            anchorX="center"
            anchorY="middle"
            letterSpacing={0.03}
          >
            {district.station.toUpperCase()}
          </Text>
        </Billboard>
      ))}
    </group>
  );
}

function tagLift(index: number, time: number, paused: boolean) {
  const here = placeAgent(index, time, paused);
  let lift = 0;
  for (let other = 0; other < index; other += 1) {
    const pose = placeAgent(other, time, paused);
    const dx = here.x - pose.x;
    const dz = here.z - pose.z;
    if (dx * dx + dz * dz < 2.4) lift += 0.42;
  }
  return lift;
}

function AgentMesh({
  agent,
  index,
  paused,
  audit,
  onFollow,
}: {
  agent: CityAgent;
  index: number;
  paused: boolean;
  audit: boolean;
  onFollow: (index: number) => void;
}) {
  const gltf = useGLTF("/models/agent.glb", false, true);
  const group = useRef<THREE.Group>(null);
  const tag = useRef<THREE.Group>(null);
  const clipName = useRef("");
  const mood = useRef<Mood>("trait");
  const scene = useMemo(() => {
    const cloned = SkeletonUtils.clone(gltf.scene);
    dress(cloned, BODY_COLORS[agent.skin.body] ?? BODY_COLORS[0]);
    applyMood(cloned, agent, "trait");
    return cloned;
  }, [agent, gltf.scene]);
  const mixer = useMemo(() => new THREE.AnimationMixer(scene), [scene]);

  useFrame((state, delta) => {
    const pose = placeAgent(index, state.clock.elapsedTime, paused);
    const nextMood = moodFor(pose.x, pose.z);
    if (nextMood !== mood.current) {
      mood.current = nextMood;
      applyMood(scene, agent, nextMood);
    }
    const nextClip = paused ? "idle" : nextMood === "happy" ? "celebrate" : "walk";
    if (clipName.current !== nextClip) {
      mixer.stopAllAction();
      const clip = gltf.animations.find((entry) => entry.name === nextClip);
      if (clip) mixer.clipAction(clip).reset().play();
      clipName.current = nextClip;
    }
    if (!paused) mixer.update(delta);
    if (group.current) {
      group.current.position.set(pose.x, pose.y, pose.z);
      group.current.rotation.y = pose.rot;
    }
    if (tag.current) tag.current.position.y = 1.55 + tagLift(index, state.clock.elapsedTime, paused);
  });

  return (
    <group ref={group}>
      <primitive
        object={scene}
        onClick={(event: ThreeEvent<MouseEvent>) => {
          event.stopPropagation();
          onFollow(index);
        }}
      />
      <group ref={tag} position={[0, 1.55, 0]}>
        <Html center sprite distanceFactor={12} zIndexRange={[8, 0]} style={{ pointerEvents: "none" }}>
          <div className={audit ? "name-tag audit" : "name-tag"}>{agent.name}</div>
        </Html>
      </group>
    </group>
  );
}

function Grade({ focusId }: { focusId: string | null }) {
  const gl = useThree((state) => state.gl);
  const scene = useThree((state) => state.scene);
  const camera = useThree((state) => state.camera);
  const size = useThree((state) => state.size);
  const focus = useRef(focusId);
  focus.current = focusId;
  const grade = useMemo(() => {
    const composer = new EffectComposer(gl, { multisampling: 0, frameBufferType: THREE.HalfFloatType });
    const normal = new NormalPass(scene, camera, { resolutionScale: 0.5 });
    const dof = new DepthOfFieldEffect(camera, { focusDistance: 18, focusRange: 8, bokehScale: 0.2, resolutionScale: 0.5 });
    const ssao = new SSAOEffect(camera, normal.texture, {
      samples: 9,
      rings: 4,
      intensity: 0.4,
      radius: 0.08,
      luminanceInfluence: 0.65,
      bias: 0.04,
    });
    composer.addPass(normal);
    composer.addPass(new RenderPass(scene, camera));
    composer.addPass(
      new EffectPass(
        camera,
        ssao,
        dof,
        new BloomEffect({ intensity: 0.35, luminanceThreshold: 0.85, luminanceSmoothing: 0.25, mipmapBlur: true }),
        new ToneMappingEffect({ mode: ToneMappingMode.ACES_FILMIC }),
      ),
    );
    return { composer, dof };
  }, [camera, gl, scene]);

  useLayoutEffect(() => {
    grade.composer.setSize(size.width, size.height);
  }, [grade, size.height, size.width]);

  useLayoutEffect(() => () => grade.composer.dispose(), [grade]);

  useFrame((_, delta) => {
    const room = DISTRICTS.find((district) => district.id === focus.current);
    if (!grade.dof.target) grade.dof.target = new THREE.Vector3();
    if (room) {
      grade.dof.target.set(room.x, 1.2, room.z);
      grade.dof.bokehScale = 1.7;
    } else {
      grade.dof.target.set(0, 1.2, 0);
      grade.dof.bokehScale = 0.15;
    }
    grade.composer.render(delta);
  }, 1);

  return null;
}

const earDirection = new THREE.Vector3();

function Ears({ paused }: { paused: boolean }) {
  const camera = useThree((state) => state.camera);
  useFrame((state) => {
    camera.getWorldDirection(earDirection);
    audio.listener(
      camera.position.x,
      camera.position.y,
      camera.position.z,
      earDirection.x,
      earDirection.y,
      earDirection.z,
    );
    if (paused) return;
    const pose = placeAgent(0, state.clock.elapsedTime, false);
    audio.blip(pose.x, 1, pose.z, state.clock.elapsedTime);
  });
  return null;
}

function CameraDriver({
  mode,
  beat,
  focusId,
  followIndex,
  paused,
}: {
  mode: "orbit" | "cutscene" | "play";
  beat: number;
  focusId: string | null;
  followIndex: number | null;
  paused: boolean;
}) {
  const camera = useThree((state) => state.camera);
  const controls = useThree((state) => state.controls) as Rig | null;
  const reduce = useRef(false);
  const wasDriving = useRef(false);
  const homing = useRef(false);
  useLayoutEffect(() => {
    reduce.current = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }, []);

  useFrame((state, delta) => {
    const rig = controls;
    if (!rig?.target || !(camera instanceof THREE.PerspectiveCamera)) return;
    const driving = mode === "cutscene" || (mode === "play" && (followIndex !== null || Boolean(focusId)));
    if (wasDriving.current && !driving) homing.current = true;
    wasDriving.current = driving;
    rig.autoRotate = false;
    if (mode === "orbit") {
      rig.enabled = false;
      aim.copy(overviewAim);
      eye.copy(overviewEye);
      const blend = reduce.current ? 1 : 1 - Math.exp(-delta * 2);
      rig.target.lerp(aim, blend);
      camera.position.lerp(eye, blend);
      camera.lookAt(rig.target);
      return;
    }
    if (!driving && homing.current) {
      rig.enabled = false;
      aim.copy(overviewAim);
      eye.copy(overviewEye);
      const blend = reduce.current ? 1 : 1 - Math.exp(-delta * 2.4);
      rig.target.lerp(aim, blend);
      camera.position.lerp(eye, blend);
      camera.lookAt(rig.target);
      if (camera.position.distanceTo(overviewEye) < 0.6) homing.current = false;
      return;
    }
    rig.enabled = !driving;
    let drive = false;
    if (mode === "cutscene") {
      const shot = BEATS[Math.min(beat, BEATS.length - 1)];
      aim.set(shot.target[0], shot.target[1], shot.target[2]);
      eye.set(shot.position[0], shot.position[1], shot.position[2]);
      drive = true;
    } else if (followIndex !== null) {
      const pose = placeAgent(followIndex, state.clock.elapsedTime, paused);
      const aheadX = Math.sin(pose.rot);
      const aheadZ = Math.cos(pose.rot);
      aim.set(pose.x, 1, pose.z);
      eye.set(pose.x - aheadX * 4.2, 3.4, pose.z - aheadZ * 4.2);
      drive = true;
    } else if (focusId) {
      const room = DISTRICTS.find((district) => district.id === focusId);
      if (room) {
        aim.set(room.x, 1.15, room.z);
        eye.set(room.x + 8, 7.2, room.z + 8);
        drive = true;
      }
    }
    if (!drive) return;
    const blend = reduce.current ? 1 : 1 - Math.exp(-delta * 3.2);
    rig.target.lerp(aim, blend);
    camera.position.lerp(eye, blend);
    camera.lookAt(rig.target);
  });
  return null;
}

export default function CityMap({
  paused,
  audit,
  mode,
  beat,
  focusId,
  followIndex,
  onPick,
  onFollow,
  onUnavailable,
}: {
  paused: boolean;
  audit: boolean;
  mode: "orbit" | "cutscene" | "play";
  beat: number;
  focusId: string | null;
  followIndex: number | null;
  onPick: (district: District) => void;
  onFollow: (index: number) => void;
  onUnavailable: () => void;
}) {
  const [dpr, setDpr] = useState<number | [number, number]>([1, 1.5]);

  return (
    <Canvas
      className="city-canvas"
      dpr={dpr}
      shadows
      camera={{ position: [overviewEye.x, overviewEye.y, overviewEye.z], fov: 25, near: 0.1, far: 700 }}
      gl={{
        antialias: false,
        alpha: false,
        powerPreference: "default",
        failIfMajorPerformanceCaveat: false,
        preserveDrawingBuffer: true,
      }}
      onCreated={({ gl }) => {
        gl.toneMapping = THREE.NoToneMapping;
        gl.shadowMap.enabled = true;
        gl.shadowMap.type = THREE.PCFSoftShadowMap;
        gl.domElement.addEventListener("webglcontextlost", (event) => {
          event.preventDefault();
          onUnavailable();
        });
      }}
    >
      <color attach="background" args={["#ffcf9e"]} />
      <World />
      <PerformanceMonitor flipflops={4} onDecline={() => setDpr(1)} onIncline={() => setDpr([1, 1.5])} />
      <Station onPick={onPick} />
      {CITY_AGENTS.map((agent, index) => (
        <AgentMesh key={agent.accountId} agent={agent} index={index} paused={paused} audit={audit} onFollow={onFollow} />
      ))}
      <Ears paused={paused} />
      <CameraDriver mode={mode} beat={beat} focusId={focusId} followIndex={followIndex} paused={paused} />
      <Grade focusId={focusId} />
      <OrbitControls
        makeDefault
        enablePan={false}
        autoRotate={false}
        minPolarAngle={0.35}
        maxPolarAngle={1.15}
        minDistance={8}
        maxDistance={180}
        target={[0, 1.2, 0]}
      />
    </Canvas>
  );
}
