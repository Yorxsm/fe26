"use client";

import { Billboard, Html, OrbitControls, PerformanceMonitor, Text, useGLTF } from "@react-three/drei";
import { DISPLAY_FONT } from "@/lib/fonts";
import { Canvas, useFrame, useThree, type ThreeEvent } from "@react-three/fiber";
import { BloomEffect, EffectComposer, EffectPass, RenderPass } from "postprocessing";
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
const OVERVIEW_DIST = 96;
const overviewEye = new THREE.Vector3(
  Math.sin(OVERVIEW_YAW) * Math.cos(OVERVIEW_PITCH) * OVERVIEW_DIST,
  Math.sin(OVERVIEW_PITCH) * OVERVIEW_DIST,
  Math.cos(OVERVIEW_YAW) * Math.cos(OVERVIEW_PITCH) * OVERVIEW_DIST,
);
const overviewAim = new THREE.Vector3(0, 1.2, 0);

const BEATS: Array<{ position: [number, number, number]; target: [number, number, number] }> = [
  { position: [-5.8, 3.2, -1.6], target: [-8.8, 1.15, -5.2] },
  { position: [12.2, 3.5, 8.6], target: [8.8, 1.2, 5.2] },
  { position: [-5.6, 3.3, 8.6], target: [-8.8, 1.2, 5.2] },
  { position: [4, 6.5, 8], target: [0, 1.2, 0] },
  { position: [0, 4.2, -11.2], target: [0, 1.4, 2] },
];

type Rig = { target: THREE.Vector3; update: () => void; autoRotate: boolean; enabled: boolean };

const aim = new THREE.Vector3();
const eye = new THREE.Vector3();

let outlineMaterial: THREE.MeshBasicMaterial | null = null;

function outlineMat() {
  if (!outlineMaterial) {
    outlineMaterial = new THREE.MeshBasicMaterial({ color: "#0c1016", side: THREE.BackSide });
  }
  return outlineMaterial;
}

function toonGradient() {
  const canvas = document.createElement("canvas");
  canvas.width = 4;
  canvas.height = 1;
  const context = canvas.getContext("2d");
  if (!context) return null;
  ["#3a3f48", "#8d93a0", "#e6e8ee", "#ffffff"].forEach((color, index) => {
    context.fillStyle = color;
    context.fillRect(index, 0, 1, 1);
  });
  const texture = new THREE.CanvasTexture(canvas);
  texture.magFilter = THREE.NearestFilter;
  texture.minFilter = THREE.NearestFilter;
  texture.generateMipmaps = false;
  return texture;
}

function toonFrom(source: THREE.Material, gradient: THREE.Texture | null) {
  const standard = source as THREE.MeshStandardMaterial;
  const color = standard.color ? standard.color.clone() : new THREE.Color("#8a939c");
  const emissive = standard.emissive ? standard.emissive.clone() : new THREE.Color(0, 0, 0);
  const strength = standard.emissiveIntensity ?? 0;
  const glowing = strength > 0.15 && emissive.r + emissive.g + emissive.b > 0.05;
  const material = new THREE.MeshToonMaterial({
    color,
    emissive: glowing ? emissive : new THREE.Color(0, 0, 0),
    emissiveIntensity: glowing ? strength : 0,
    gradientMap: gradient ?? undefined,
  });
  material.name = source.name;
  if (glowing) material.toneMapped = false;
  return material;
}

function addOutline(mesh: THREE.Mesh) {
  const skinned = mesh as THREE.SkinnedMesh;
  if (skinned.isSkinnedMesh) {
    const hull = new THREE.SkinnedMesh(skinned.geometry, outlineMat());
    hull.bind(skinned.skeleton, skinned.bindMatrix);
    hull.scale.setScalar(1.045);
    hull.frustumCulled = false;
    hull.raycast = () => undefined;
    skinned.add(hull);
    return;
  }
  const hull = new THREE.Mesh(mesh.geometry, outlineMat());
  hull.scale.setScalar(1.03);
  hull.raycast = () => undefined;
  mesh.add(hull);
}

function dress(root: THREE.Object3D, gradient: THREE.Texture | null, recolor?: string) {
  const meshes: THREE.Mesh[] = [];
  root.traverse((node) => {
    const mesh = node as THREE.Mesh;
    if (mesh.isMesh) meshes.push(mesh);
  });
  meshes.forEach((mesh) => {
    const sources = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
    const next = sources.map((source) => toonFrom(source, gradient));
    if (recolor) {
      next.forEach((material) => {
        if (material.name === "Body") material.color.set(recolor);
      });
    }
    mesh.material = next.length === 1 ? next[0] : next;
    const prefix = mesh.name.split(".")[0];
    const district = ROOM_PREFIX[prefix];
    if (district) mesh.userData.district = district;
    addOutline(mesh);
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

function Station({
  gradient,
  onPick,
}: {
  gradient: THREE.Texture | null;
  onPick: (district: District) => void;
}) {
  const gltf = useGLTF("/models/station.glb", false, true);
  const scene = useMemo(() => dress(gltf.scene.clone(true), gradient), [gltf.scene, gradient]);
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
        <Billboard key={district.id} position={[district.x, 2.85, district.z]}>
          <mesh position={[0, 0, -0.04]}>
            <planeGeometry args={[Math.max(1.4, district.station.length * 0.28), 0.5]} />
            <meshStandardMaterial color="#2a241c" roughness={0.85} />
          </mesh>
          <Text
            font={DISPLAY_FONT}
            fontSize={0.24}
            color="#f3e6c8"
            anchorX="center"
            anchorY="middle"
            letterSpacing={0.04}
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
  gradient,
  onFollow,
}: {
  agent: CityAgent;
  index: number;
  paused: boolean;
  audit: boolean;
  gradient: THREE.Texture | null;
  onFollow: (index: number) => void;
}) {
  const gltf = useGLTF("/models/agent.glb", false, true);
  const group = useRef<THREE.Group>(null);
  const tag = useRef<THREE.Group>(null);
  const clipName = useRef("");
  const mood = useRef<Mood>("trait");
  const scene = useMemo(() => {
    const cloned = SkeletonUtils.clone(gltf.scene);
    dress(cloned, gradient, BODY_COLORS[agent.skin.body] ?? BODY_COLORS[0]);
    applyMood(cloned, agent, "trait");
    return cloned;
  }, [agent, gltf.scene, gradient]);
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

function ScreenBloom() {
  const gl = useThree((state) => state.gl);
  const scene = useThree((state) => state.scene);
  const camera = useThree((state) => state.camera);
  const size = useThree((state) => state.size);
  const composer = useMemo(() => {
    const next = new EffectComposer(gl, { multisampling: 0, frameBufferType: THREE.HalfFloatType });
    next.addPass(new RenderPass(scene, camera));
    next.addPass(
      new EffectPass(
        camera,
        new BloomEffect({
          intensity: 0.45,
          luminanceThreshold: 0.9,
          luminanceSmoothing: 0.2,
          mipmapBlur: true,
        }),
      ),
    );
    return next;
  }, [camera, gl, scene]);

  useLayoutEffect(() => {
    composer.setSize(size.width, size.height);
  }, [composer, size.height, size.width]);

  useLayoutEffect(() => () => composer.dispose(), [composer]);

  useFrame((_, delta) => {
    composer.render(delta);
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
        eye.set(room.x + 3.4, 4.4, room.z + 4.6);
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
  const gradient = useMemo(() => toonGradient(), []);
  const [dpr, setDpr] = useState<number | [number, number]>([1, 1.5]);

  return (
    <Canvas
      className="city-canvas"
      dpr={dpr}
      camera={{ position: [overviewEye.x, overviewEye.y, overviewEye.z], fov: 25, near: 0.1, far: 600 }}
      gl={{
        antialias: false,
        alpha: false,
        powerPreference: "default",
        failIfMajorPerformanceCaveat: false,
        preserveDrawingBuffer: true,
      }}
      onCreated={({ gl }) => {
        gl.domElement.addEventListener("webglcontextlost", (event) => {
          event.preventDefault();
          onUnavailable();
        });
      }}
    >
      <color attach="background" args={["#f3c7a4"]} />
      <fog attach="fog" args={["#f0cbb4", 70, 220]} />
      <hemisphereLight args={["#ffe0c2", "#8fb8ff", 0.85]} />
      <directionalLight position={[30, 40, 12]} intensity={1.5} color="#ffd2a8" />
      <PerformanceMonitor flipflops={4} onDecline={() => setDpr(1)} onIncline={() => setDpr([1, 1.5])} />
      <mesh rotation={[-Math.PI / 2, 0, 0]} position={[0, -6, 0]}>
        <circleGeometry args={[220, 48]} />
        <meshStandardMaterial color="#f6d7c4" roughness={1} />
      </mesh>
      <Station gradient={gradient} onPick={onPick} />
      {CITY_AGENTS.map((agent, index) => (
        <AgentMesh
          key={agent.accountId}
          agent={agent}
          index={index}
          paused={paused}
          audit={audit}
          gradient={gradient}
          onFollow={onFollow}
        />
      ))}
      <Ears paused={paused} />
      <CameraDriver mode={mode} beat={beat} focusId={focusId} followIndex={followIndex} paused={paused} />
      <ScreenBloom />
      <OrbitControls
        makeDefault
        enablePan={false}
        autoRotate={false}
        minPolarAngle={0.35}
        maxPolarAngle={1.15}
        minDistance={8}
        maxDistance={150}
        target={[0, 1.2, 0]}
      />
    </Canvas>
  );
}
