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
import { Interiors } from "@/components/Interiors";
import { Nature } from "@/components/Nature";
import { World } from "@/components/World";
import { useLayoutEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import { SkeletonUtils } from "three-stdlib";
import { audio } from "@/lib/audio";
import { AGENT_HEAD, AGENT_SCALE, CITY_AGENTS, DISTRICTS, EXTERIOR_SHOTS, FACE_SHOT, INTERIOR_SHOTS, LINEUP_SHOT, LINEUP_SPOTS, STREET_SHOT, WALK_SHOT, placeAgent, poseClearOf, type CityAgent, type District } from "@/lib/city";
import { PAINT_COLORS } from "@/lib/skin";
import { expressionForClip, mountFace, redrawFace, type FaceExpression } from "@/lib/faceCanvas";
import { lineupSkin, nodeVisible, paintFinish, screenColor, type Mood, type PaintFinish, type SkinTraits } from "@/lib/skin";

useGLTF.preload("/models/kits.glb", false, true);
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

/** Free-camera home. The lobby uses titleEye so the lockup sits on sky. */
const overviewEye = new THREE.Vector3(-32, 16, 58);
const overviewAim = new THREE.Vector3(0, 6, -4);
/**
 * Lobby. Almost level, from above the roofs, so the horizon lands near
 * mid-frame and the lit plaza fills the lower half. Mobile looks a little
 * higher because the title block is a larger share of a tall screen.
 */
const titleEye = new THREE.Vector3(-22, 12.5, 36);
const titleAim = new THREE.Vector3(18, 11.1, -6);
const titleEyeMobile = new THREE.Vector3(-22, 12.5, 36);
const titleAimMobile = new THREE.Vector3(18, 14.9, -6);

const BEAT_PREFIX = ["vault", "market", "war", "forge", "stadium"];
const BEAT_IDS = ["vault-quarter", "trading-village", "war-room", "skin-forge", "cup-stadium"];
const BEATS: Array<{ position: [number, number, number]; target: [number, number, number] }> = [
  { position: INTERIOR_SHOTS["vault-quarter"].eye, target: INTERIOR_SHOTS["vault-quarter"].aim },
  { position: INTERIOR_SHOTS["trading-village"].eye, target: INTERIOR_SHOTS["trading-village"].aim },
  { position: INTERIOR_SHOTS["war-room"].eye, target: INTERIOR_SHOTS["war-room"].aim },
  { position: INTERIOR_SHOTS["skin-forge"].eye, target: INTERIOR_SHOTS["skin-forge"].aim },
  { position: [-8, 7, -20], target: [-8, 2.2, -48] },
];

type Rig = { target: THREE.Vector3; update: () => void; autoRotate: boolean; enabled: boolean };

const aim = new THREE.Vector3();
const eye = new THREE.Vector3();
const driftOffset = new THREE.Vector3();
const driftUp = new THREE.Vector3(0, 1, 0);
/** Body cameras and lights were tuned at scale 5.3. */
const fit = AGENT_SCALE / 5.3;
const FULL_CAP = 24;
const CULL_DISTANCE = 140;
const impostorDummy = new THREE.Object3D();
const impostorColor = new THREE.Color();
const projected = new THREE.Vector3();

function readCrowd(): { count: number; measure: boolean; full: boolean } {
  if (typeof window === "undefined") return { count: CITY_AGENTS.length, measure: false, full: false };
  const params = new URLSearchParams(window.location.search);
  const raw = params.get("agents");
  if (raw === null) return { count: CITY_AGENTS.length, measure: false, full: false };
  const parsed = Number(raw);
  if (!Number.isFinite(parsed) || parsed < 1) return { count: CITY_AGENTS.length, measure: false, full: false };
  return { count: Math.min(5000, Math.floor(parsed)), measure: true, full: params.get("detail") === "all" };
}

/** Kenney atlases keep a solid black band for a ground-shadow card. Drop those texels. */
function dropBakedShadow(material: THREE.Material) {
  const standard = material as THREE.MeshStandardMaterial;
  if (!standard.isMeshStandardMaterial || !standard.map) return;
  standard.onBeforeCompile = (shader) => {
    shader.fragmentShader = shader.fragmentShader.replace(
      "#include <color_fragment>",
      `#include <color_fragment>
       if (diffuseColor.r < 0.05 && diffuseColor.g < 0.05 && diffuseColor.b < 0.05) discard;`,
    );
  };
  standard.customProgramCacheKey = () => "fe2vi-drop-baked-shadow";
}

/** glTFLoader strips "." from Object3D names. The authored name stays on userData. */
function sourceName(node: THREE.Object3D): string {
  let current: THREE.Object3D | null = node;
  while (current) {
    const stored = current.userData.name;
    if (typeof stored === "string" && stored.includes(".")) return stored;
    current = current.parent;
  }
  return node.name;
}

function isStandard(material: THREE.Material): material is THREE.MeshStandardMaterial {
  return (material as THREE.MeshStandardMaterial).isMeshStandardMaterial === true;
}

function paintScreen(root: THREE.Object3D, name: string, draw: (ctx: CanvasRenderingContext2D, width: number, height: number) => void) {
  const mesh = root.getObjectByName(name) as THREE.Mesh | undefined;
  if (!mesh || !(mesh.material instanceof THREE.MeshStandardMaterial)) return;
  const canvas = document.createElement("canvas");
  canvas.width = 512;
  canvas.height = 320;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  draw(ctx, canvas.width, canvas.height);
  const tex = new THREE.CanvasTexture(canvas);
  tex.colorSpace = THREE.SRGBColorSpace;
  const mat = mesh.material.clone();
  mat.map = tex;
  mat.emissiveMap = tex;
  mat.emissive = new THREE.Color("#d8fff0");
  mat.emissiveIntensity = 0.4;
  mesh.material = mat;
}

function paintMarket(ctx: CanvasRenderingContext2D, width: number, height: number) {
  ctx.fillStyle = "#101418";
  ctx.fillRect(0, 0, width, height);
  ctx.fillStyle = "#f3e6c8";
  ctx.font = "700 30px sans-serif";
  ctx.fillText("MARKET LEADERBOARD", 24, 46);
  ctx.font = "500 24px sans-serif";
  ctx.fillStyle = "#3ee07a";
  [
    ["1", "Mason", "12.4"],
    ["2", "Scout", "9.1"],
    ["3", "Tape", "7.8"],
    ["4", "Pike", "4.2"],
  ].forEach((row, index) => {
    ctx.fillText(`${row[0]}   ${row[1]}`, 28, 100 + index * 36);
    ctx.fillText(row[2], 380, 100 + index * 36);
  });
  ctx.beginPath();
  ctx.lineWidth = 4;
  [
    [36, 290],
    [120, 260],
    [200, 270],
    [280, 220],
    [360, 200],
    [460, 150],
  ].forEach(([x, y], index) => (index === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y)));
  ctx.stroke();
}

function paintStadium(ctx: CanvasRenderingContext2D, width: number, height: number) {
  ctx.fillStyle = "#101418";
  ctx.fillRect(0, 0, width, height);
  ctx.fillStyle = "#f3e6c8";
  ctx.font = "700 28px sans-serif";
  ctx.fillText("TRADE LEAGUE", 24, 42);
  ctx.font = "500 20px sans-serif";
  ctx.fillText("SEASON 1", 24, 72);
  ctx.fillStyle = "#3ee0b5";
  ctx.font = "500 26px sans-serif";
  [
    ["Mason", "18"],
    ["Scout", "14"],
    ["Tape", "11"],
    ["Pike", "7"],
  ].forEach((row, index) => {
    ctx.beginPath();
    ctx.arc(40, 120 + index * 42, 8, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillText(row[0], 64, 128 + index * 42);
    ctx.fillText(row[1], 400, 128 + index * 42);
  });
}

function paintWarMap(ctx: CanvasRenderingContext2D, width: number, height: number) {
  ctx.fillStyle = "#071418";
  ctx.fillRect(0, 0, width, height);
  ctx.strokeStyle = "#2fe6e6";
  ctx.lineWidth = 2;
  for (let i = 0; i < 8; i += 1) {
    ctx.strokeRect(40 + (i % 4) * 110, 40 + Math.floor(i / 4) * 120, 90, 80);
    ctx.fillStyle = "#2fe6e6";
    ctx.font = "16px sans-serif";
    ctx.fillText(`NODE ${i + 1}`, 52 + (i % 4) * 110, 84 + Math.floor(i / 4) * 120);
  }
}

function Smoke({ position }: { position: [number, number, number] }) {
  const group = useRef<THREE.Group>(null);
  useFrame((state) => {
    const puffs = group.current?.children ?? [];
    const time = state.clock.elapsedTime;
    puffs.forEach((puff, index) => {
      const rise = (time * 0.18 + index * 0.34) % 1;
      puff.position.y = rise * 5.5;
      const scale = 0.35 + rise * 1.5;
      puff.scale.setScalar(scale);
      const mat = (puff as THREE.Mesh).material as THREE.MeshStandardMaterial;
      mat.opacity = 0.28 * (1 - rise);
    });
  });
  return (
    <group ref={group} position={position}>
      {[0, 1, 2, 3].map((index) => (
        <mesh key={index}>
          <sphereGeometry args={[0.42, 8, 6]} />
          <meshStandardMaterial color="#efe8dc" transparent opacity={0.2} depthWrite={false} roughness={1} />
        </mesh>
      ))}
    </group>
  );
}

const INTERIOR_IDS = new Set(["vault-quarter", "trading-village", "war-room", "skin-forge"]);

const BUILDING_GRADE: Record<string, { tint: string; rough: number; metal: number }> = {
  vault: { tint: "#d4a05a", rough: 0.78, metal: 0.02 },
  market: { tint: "#c4622a", rough: 0.82, metal: 0.02 },
  war: { tint: "#3a4c5e", rough: 0.22, metal: 0.52 },
  forge: { tint: "#b33a22", rough: 0.88, metal: 0.02 },
  stadium: { tint: "#c9a36a", rough: 0.8, metal: 0.03 },
  pier: { tint: "#c4a36c", rough: 0.84, metal: 0.02 },
  club: { tint: "#c4a36c", rough: 0.84, metal: 0.02 },
  oracle: { tint: "#c4a36c", rough: 0.84, metal: 0.02 },
};

const gradedAtlases = new Map<string, THREE.Texture>();

/** Kenney's commercial atlas paints walls lilac. Swap those texels for a real wall color and leave trim alone. */
function warmAtlas(source: THREE.Texture, key: string, tint: string): THREE.Texture {
  const cached = gradedAtlases.get(key);
  if (cached) return cached;
  const image = source.image as CanvasImageSource & { width?: number; height?: number };
  if (!image?.width || !image.height) return source;
  const canvas = document.createElement("canvas");
  canvas.width = image.width;
  canvas.height = image.height;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) return source;
  ctx.drawImage(image, 0, 0);
  const pixels = ctx.getImageData(0, 0, canvas.width, canvas.height);
  const data = pixels.data;
  const hex = Number.parseInt(tint.slice(1), 16);
  const wr = (hex >> 16) & 255;
  const wg = (hex >> 8) & 255;
  const wb = hex & 255;
  for (let i = 0; i < data.length; i += 4) {
    const r = data[i] / 255;
    const g = data[i + 1] / 255;
    const b = data[i + 2] / 255;
    const max = Math.max(r, g, b);
    const min = Math.min(r, g, b);
    const luma = (r + g + b) / 3;
    const lilac = b + 0.02 >= r && b + 0.02 >= g && max - min < 0.36 && luma > 0.12 && luma < 0.92;
    if (r < 0.08 && g < 0.08 && b < 0.08) {
      data[i + 3] = 0;
      continue;
    }
    if (!lilac) continue;
    const shade = 0.42 + luma * 0.9;
    data[i] = Math.min(255, Math.round(wr * shade));
    data[i + 1] = Math.min(255, Math.round(wg * shade));
    data[i + 2] = Math.min(255, Math.round(wb * shade));
  }
  ctx.putImageData(pixels, 0, 0);
  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  texture.flipY = source.flipY;
  texture.wrapS = source.wrapS;
  texture.wrapT = source.wrapT;
  texture.repeat.copy(source.repeat);
  texture.offset.copy(source.offset);
  texture.anisotropy = source.anisotropy;
  texture.needsUpdate = true;
  gradedAtlases.set(key, texture);
  return texture;
}

function buildingGrade(name: string) {
  if (!name.startsWith("kit.")) return null;
  if (/light|flag|crane|fence|cone|scaffold|plank|sign|gantry|flood|dish/.test(name)) return null;
  return BUILDING_GRADE[name.split(".")[1]] ?? null;
}

function Kits({
  onPick,
  focusId,
  frameMode,
  beatId,
}: {
  onPick: (district: District) => void;
  focusId: string | null;
  frameMode: "in" | "out";
  beatId: string | null;
}) {
  const gltf = useGLTF("/models/kits.glb", false, true);
  const scene = useMemo(() => {
    const cloned = gltf.scene.clone(true);
    cloned.traverse((node) => {
      const label = sourceName(node);
      const key = label.startsWith("kit.") ? label.split(".")[1] : "";
      if (key && ROOM_PREFIX[key]) node.userData.district = ROOM_PREFIX[key];
      const mesh = node as THREE.Mesh;
      if (!mesh.isMesh) return;
      mesh.castShadow = true;
      mesh.receiveShadow = true;
      mesh.frustumCulled = false;
      const grade = buildingGrade(label);
      if (grade && mesh.material) {
        const atlasKey = key || "kit";
        const source = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
        const copies = source.map((mat) => {
          const copy = mat.clone();
          if (isStandard(copy)) {
            copy.roughness = grade.rough;
            copy.metalness = grade.metal;
            copy.envMapIntensity = grade.metal > 0.4 ? 0.35 : 0.08;
            if (copy.map) {
              copy.map = warmAtlas(copy.map, atlasKey, grade.tint);
              copy.color.set("#ffffff");
              copy.alphaTest = 0.4;
              copy.transparent = false;
            } else {
              copy.color.set(grade.tint);
            }
          }
          return copy;
        });
        mesh.material = Array.isArray(mesh.material) ? copies : copies[0];
      }
      if (/lights|flood|gantry/.test(label) && mesh.material) {
        const source = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
        const copies = source.map((mat) => {
          const copy = mat.clone();
          if (isStandard(copy)) {
            copy.emissive = new THREE.Color("#ffe6b0");
            copy.emissiveIntensity = 0.7;
          }
          return copy;
        });
        mesh.material = Array.isArray(mesh.material) ? copies : copies[0];
      }
      const finished = (Array.isArray(mesh.material) ? mesh.material : [mesh.material]) as THREE.Material[];
      finished.forEach(dropBakedShadow);
    });
    return cloned;
  }, [gltf.scene]);
  useLayoutEffect(() => {
    const openId = beatId ?? (frameMode === "in" ? focusId : null);
    scene.traverse((node) => {
      const id = node.userData.district as string | undefined;
      if (!id) return;
      node.visible = !(openId === id && INTERIOR_IDS.has(id));
    });
  }, [scene, focusId, frameMode, beatId]);
  return (
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
  );
}

function paintColor(material: THREE.MeshStandardMaterial, finish: PaintFinish, emissive = false) {
  const copy = material.clone();
  copy.name = material.name;
  copy.color.set(finish.color);
  if (finish.metal !== undefined) copy.metalness = finish.metal;
  if (finish.rough !== undefined) copy.roughness = finish.rough;
  if (emissive && copy.emissive) {
    copy.emissive.set(finish.color);
    copy.emissiveIntensity = 1.8;
  }
  return copy;
}

function dress(root: THREE.Object3D, finish?: PaintFinish, screen?: string) {
  const cooled = new WeakSet<THREE.Material>();
  const meshes: THREE.Mesh[] = [];
  root.traverse((node) => {
    const mesh = node as THREE.Mesh;
    if (mesh.isMesh) meshes.push(mesh);
  });
  meshes.forEach((mesh) => {
    let sources = (Array.isArray(mesh.material) ? mesh.material : [mesh.material]) as THREE.MeshStandardMaterial[];
    if (finish) {
      sources = sources.map((material) => {
        if (material.name === "Body" && material.color) return paintColor(material, finish);
        if (screen && material.name === "Screen") {
          const glass = material.clone();
          glass.name = "Screen";
          glass.color.set("#0a0a0b");
          glass.emissive.set("#000000");
          glass.emissiveIntensity = 0;
          glass.metalness = 0.62;
          glass.roughness = 0.14;
          return glass;
        }
        return material;
      });
      mesh.material = Array.isArray(mesh.material) ? sources : sources[0];
    }
    sources.forEach((material) => {
      if (!("envMapIntensity" in material)) return;
      const metal = "metalness" in material ? material.metalness : 0;
      material.envMapIntensity = metal > 0.45 ? 0.16 : 0.03;
      if (!finish && material.color && !cooled.has(material)) {
        material.color.multiply(new THREE.Color(0.62, 0.66, 0.72));
        cooled.add(material);
      }
    });
    const label = sourceName(mesh);
    if (label.startsWith("terrain.")) {
      const lit = sources.map((material) => {
        const copy = material.clone();
        copy.color.set("#2c4a40");
        copy.emissive = new THREE.Color("#1c342c");
        copy.emissiveIntensity = 1.15;
        return copy;
      });
      mesh.material = Array.isArray(mesh.material) ? lit : lit[0];
    }
    const prefix = label.split(".")[0];
    const district = ROOM_PREFIX[prefix];
    if (district) mesh.userData.district = district;
    mesh.castShadow = true;
    mesh.receiveShadow = true;
    mesh.frustumCulled = false;
    if (label.startsWith("tree.")) mesh.visible = false;
    if (ROOM_PREFIX[prefix] && !label.endsWith(".pad")) mesh.visible = false;
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

function applyLook(root: THREE.Object3D, skin: SkinTraits, mood: Mood) {
  root.traverse((node) => {
    const shown = nodeVisible(sourceName(node), skin, mood);
    if (shown !== null) node.visible = shown;
  });
}

function Station({
  onPick,
  focusId,
  beatPrefix,
}: {
  onPick: (district: District) => void;
  focusId: string | null;
  beatPrefix: string | null;
}) {
  const gltf = useGLTF("/models/station.glb", false, true);
  const scene = useMemo(() => dress(gltf.scene.clone(true)), [gltf.scene]);
  useLayoutEffect(() => {
    paintScreen(scene, "market.screen", paintMarket);
    paintScreen(scene, "stadium.screen", paintStadium);
    paintScreen(scene, "war.map", paintWarMap);
  }, [scene]);
  useFrame((state, delta) => {
    const wheel = scene.getObjectByName("vault.wheel");
    if (wheel) wheel.rotateY(delta * 0.35);
    const openPrefix = Object.entries(ROOM_PREFIX).find(([, id]) => id === focusId)?.[0] ?? beatPrefix;
    scene.traverse((node) => {
      const bits = sourceName(node).split(".");
      if (!bits.includes("roof")) return;
      // The market kit is the exterior. The shell roof sits in front of it and reads as a black slab.
      node.visible = bits[0] !== openPrefix && bits[0] !== "market";
    });
    const holo = scene.getObjectByName("war.holo");
    if (holo) holo.rotateY(delta * 0.8);
    const beacon = scene.getObjectByName("war.beacon") as THREE.Mesh | undefined;
    if (beacon) {
      beacon.rotateY(delta * 2.2);
      const mat = beacon.material as THREE.MeshStandardMaterial;
      if (mat?.emissiveIntensity !== undefined) mat.emissiveIntensity = 2 + Math.sin(state.clock.elapsedTime * 6) * 1.5;
    }
  });
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
        <Billboard key={district.id} position={[district.x, 11, district.z]}>
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
    if (dx * dx + dz * dz < 2.4 * fit * fit) lift += 0.42 * fit;
  }
  return lift;
}

function displaySkin(agent: CityAgent, index: number, poster: string | null): SkinTraits {
  if (poster === "lineup" || poster === "face" || poster === "gear" || poster === "front6" || poster === "q34") return lineupSkin(index);
  if (poster === "expressions") {
    return { ...lineupSkin(0), eyes: 0, headwear: 5, face_item: 3, back_item: 1, screen: 0, paint: 0, role: 0 };
  }
  if (poster === "eyes") {
    return { ...lineupSkin(0), eyes: index % 6, headwear: 5, face_item: 3, back_item: 1, screen: index % 8, paint: 0, role: 0 };
  }
  if (poster === "trails") return { ...lineupSkin(index), trail: index % 4 };
  if (poster === "skins") {
    return {
      ...lineupSkin(index % 6),
      paint: index % 12,
      screen: index % 8,
      eyes: index % 6,
      headwear: index % 8,
      face_item: index % 4,
      back_item: index % 4,
      emblem: index % 6,
    };
  }
  if (poster === "walk") return lineupSkin(4);
  return { ...agent.skin, role: index % 6 };
}

function ParticleTrail({ kind }: { kind: number }) {
  const mesh = useRef<THREE.InstancedMesh>(null);
  const count = 18;
  useFrame(({ clock }) => {
    const field = mesh.current;
    if (!field) return;
    const dummy = new THREE.Object3D();
    const t = clock.elapsedTime;
    for (let i = 0; i < count; i += 1) {
      const age = (t * 0.85 + i / count) % 1;
      const back = 0.55 + age * 2.4;
      let x = Math.sin(i * 2.1 + t * 3) * 0.18;
      let y = 0.55 + age * (kind === 3 ? 1.8 : 1.1);
      if (kind === 1) {
        x = Math.sin(i) * 0.22;
        y = 0.7 + Math.sin(t * 4 + i) * 0.2;
      }
      if (kind === 2) {
        x = ((i * 5) % 7) * 0.08 - 0.24;
        y = 0.5 + ((i * 3) % 5) * 0.22;
      }
      dummy.position.set(x, y, -back);
      const size = kind === 3 ? 0.08 + age * 0.22 : kind === 1 ? 0.12 : 0.07;
      dummy.scale.setScalar(size * (1 - age * 0.65));
      dummy.rotation.set(kind === 1 ? t * 3 + i : 0, 0, 0);
      dummy.updateMatrix();
      field.setMatrixAt(i, dummy.matrix);
    }
    field.instanceMatrix.needsUpdate = true;
  });
  const color = ["#c94a1a", "#e2b340", "#2fe6e6", "#b8b8c4"][kind % 4];
  return (
    <instancedMesh ref={mesh} args={[undefined, undefined, count]}>
      {kind === 1 ? <cylinderGeometry args={[1, 1, 0.22, 8]} /> : kind === 2 ? <boxGeometry args={[1, 0.3, 1]} /> : <sphereGeometry args={[1, 6, 5]} />}
      <meshStandardMaterial color={color} emissive={color} emissiveIntensity={kind === 3 ? 0.15 : 1.6} roughness={0.35} />
    </instancedMesh>
  );
}

function AgentMesh({
  agent,
  index,
  paused,
  audit,
  focusId,
  lineup,
  poster,
  onFollow,
  rim = true,
}: {
  agent: CityAgent;
  index: number;
  paused: boolean;
  audit: boolean;
  focusId: string | null;
  lineup: boolean;
  poster: string | null;
  onFollow: (index: number) => void;
  rim?: boolean;
}) {
  const gltf = useGLTF("/models/agent.glb", false, true);
  const group = useRef<THREE.Group>(null);
  const tag = useRef<THREE.Group>(null);
  const clipName = useRef("");
  const clipAction = useRef<THREE.AnimationAction | null>(null);
  const mood = useRef<Mood>("trait");
  const skin = useMemo(() => displaySkin(agent, index, poster), [agent, index, poster]);
  const scene = useMemo(() => {
    const cloned = SkeletonUtils.clone(gltf.scene);
    dress(cloned, paintFinish(skin.paint), screenColor(skin.screen));
    applyLook(cloned, skin, "trait");
    cloned.traverse((node) => {
      const mesh = node as THREE.Mesh;
      if (mesh.isMesh) mesh.frustumCulled = true;
    });
    return cloned;
  }, [gltf.scene, skin]);
  const face = useMemo(() => mountFace(scene, sourceName), [scene]);
  const blink = useRef({ next: 1 + (index % 5) * 0.7, amount: 0, active: 0 });
  const saccade = useRef({ next: 0.4 + (index % 4) * 0.3, x: 0, y: 0 });
  const mixer = useMemo(() => new THREE.AnimationMixer(scene), [scene]);

  useFrame((state, delta) => {
    const walking = poseClearOf(placeAgent(index, state.clock.elapsedTime, paused), focusId, index);
    let pose: { x: number; y: number; z: number; rot: number } = walking;
    if (poster === "face") pose = { x: 0, y: 0, z: 0, rot: 0 };
    else if ((poster === "lineup" || lineup) && index < LINEUP_SPOTS.length) {
      pose = { x: LINEUP_SPOTS[index][0], y: 0, z: LINEUP_SPOTS[index][1], rot: 0 };
    } else if ((poster === "front6" || poster === "q34") && index < 6) {
      pose = { x: -7.25 + index * 2.9, y: 0, z: 0, rot: poster === "q34" ? 0.7 : 0 };
    } else if (poster === "expressions") pose = { x: -5.4 + index * 3.6, y: 0, z: 0, rot: 0 };
    else if (poster === "eyes") {
      const col = index % 3;
      const row = Math.floor(index / 3);
      pose = { x: (col - 1) * 4.4, y: 0, z: row === 0 ? 1.4 : -3.4, rot: 0 };
    } else if (poster === "gear" && index < LINEUP_SPOTS.length) {
      pose = { x: -10.5 + index * 4.2, y: 0, z: 0, rot: 0.55 };
    } else if (poster === "trails") pose = { x: -6 + index * 4, y: 0, z: 0, rot: 0 };
    else if (poster === "walk") pose = { x: -7.5 + index * 5, y: 0, z: 0, rot: 0 };
    else if (poster === "skins") {
      const col = index % 4;
      const row = Math.floor(index / 4);
      pose = { x: (col - 1.5) * 7, y: 0, z: (1 - row) * 7, rot: 0 };
    } else if (poster === "street" && index < 3) {
      const x = -15 + index * 3.4;
      const z = 11 - index * 2.1;
      pose = { x, y: 0, z, rot: Math.atan2(-18 - x, 14 - z) };
    }
    const nextMood = lineup || poster ? "trait" : moodFor(pose.x, pose.z);
    if (nextMood !== mood.current) {
      mood.current = nextMood;
      applyLook(scene, skin, nextMood);
    }
    const speed = "speed" in walking && typeof walking.speed === "number" ? walking.speed : 0;
    const planted = !poster && speed < 0.4;
    if (poster === "walk" || poster === "expressions") {
      const hold = poster === "expressions" ? ["idle", "walk", "celebrate", "caught"][index] ?? "idle" : "walk";
      const token = `${hold}-hold`;
      if (clipName.current !== token) {
        mixer.stopAllAction();
        const clip = gltf.animations.find((entry) => entry.name === hold);
        const action = clip ? mixer.clipAction(clip) : null;
        if (action && clip) {
          action.play();
          action.paused = true;
          const mark = hold === "celebrate" ? 0.35 : hold === "walk" ? (poster === "walk" ? index / 4 : 0.25) : 0.1;
          action.time = mark * clip.duration;
        }
        clipAction.current = action;
        clipName.current = token;
      }
    } else {
      const nextClip = paused || lineup || poster || planted ? "idle" : nextMood === "happy" ? "celebrate" : nextMood === "alarm" ? "caught" : "walk";
      if (clipName.current !== nextClip) {
        mixer.stopAllAction();
        const clip = gltf.animations.find((entry) => entry.name === nextClip);
        clipAction.current = clip ? mixer.clipAction(clip).reset().play() : null;
        clipName.current = nextClip;
      }
      if (clipAction.current) clipAction.current.timeScale = nextClip === "walk" ? Math.min(3.2, Math.max(0.25, speed / 1.7)) : 1;
    }
    if (!paused || poster === "walk" || poster === "expressions") mixer.update(poster === "walk" || poster === "expressions" ? 0 : delta);
    const shown =
      poster === "face" ? index === 0
      : poster === "front6" || poster === "q34" ? index < 6
      : poster === "expressions" || poster === "walk" || poster === "trails" ? index < 4
      : poster === "eyes" ? index < 6
      : poster === "skins" ? index < 12
      : poster === "street" ? index < 8
      : !lineup || index < LINEUP_SPOTS.length;
    if (group.current) {
      group.current.visible = shown;
      group.current.position.set(pose.x, pose.y, pose.z);
      group.current.rotation.y = pose.rot;
    }
    if (tag.current) {
      tag.current.visible = poster !== "face" && poster !== "walk" && poster !== "skins" && poster !== "trails" && poster !== "expressions" && poster !== "eyes" && poster !== "front6" && poster !== "q34";
      tag.current.position.y = AGENT_SCALE * 1.05 + tagLift(index, state.clock.elapsedTime, paused);
    }
    if (face && group.current?.visible) {
      const now = state.clock.elapsedTime + index * 0.37;
      if (now >= blink.current.next && blink.current.active <= 0) {
        blink.current.active = 0.14;
        blink.current.next = now + 3 + ((index * 1.7) % 3);
      }
      if (blink.current.active > 0) blink.current.active = Math.max(0, blink.current.active - delta);
      const shut = poster === "expressions" || poster === "eyes" || blink.current.active <= 0 ? 0 : Math.sin((1 - blink.current.active / 0.14) * Math.PI);
      if (now >= saccade.current.next) {
        saccade.current.x = ((index * 13 + Math.floor(now * 2)) % 5) / 2 - 1;
        saccade.current.y = ((index * 7 + Math.floor(now)) % 5) / 2 - 1;
        saccade.current.next = now + 0.8 + (index % 3) * 0.4;
      }
      const cam = state.camera.position;
      const origin = group.current.position;
      let tx = cam.x;
      let ty = cam.y;
      let tz = cam.z;
      let best = cam.distanceToSquared(origin);
      if (!poster) {
        for (let other = 0; other < CITY_AGENTS.length; other += 1) {
          if (other === index) continue;
          const there = placeAgent(other, state.clock.elapsedTime, paused);
          const dx = there.x - origin.x;
          const dz = there.z - origin.z;
          const dist = dx * dx + dz * dz;
          if (dist < best && dist > 1) {
            best = dist;
            tx = there.x;
            ty = AGENT_HEAD * (3.3 / 3.4);
            tz = there.z;
          }
        }
      }
      const fx = Math.sin(pose.rot);
      const fz = Math.cos(pose.rot);
      const side = (tx - origin.x) * fz - (tz - origin.z) * fx;
      const ahead = (tx - origin.x) * fx + (tz - origin.z) * fz;
      const lookX = Math.max(-1, Math.min(1, side / (Math.abs(ahead) + 6) * 4 + saccade.current.x * 0.25));
      const lookY = Math.max(-1, Math.min(1, (ty - (origin.y + AGENT_HEAD)) / (6 * fit) + saccade.current.y * 0.2));
      const forced: FaceExpression[] = ["calm", "focused", "happy", "shocked"];
      const expression: FaceExpression = poster === "expressions" ? forced[index] ?? "calm" : poster === "eyes" ? "calm" : expressionForClip(clipName.current.replace("-hold", ""));
      face.time.value = now;
      redrawFace(face, { style: skin.eyes, expression, blink: shut, lookX, lookY, color: screenColor(skin.screen) });
    }
  });

  return (
    <group ref={group}>
      <mesh rotation={[-Math.PI / 2, 0, 0]} position={[0, 0.03, 0]}>
        <circleGeometry args={[1.15 * fit, 20]} />
        <meshBasicMaterial color="#05060a" transparent opacity={0.38} depthWrite={false} />
      </mesh>
      {rim ? <pointLight position={[0, 3.6 * fit, -2.6 * fit]} color="#9eb4ee" intensity={2.4} distance={9 * fit} decay={2} /> : null}
      {poster === "trails" || (!poster && !lineup) ? (
        <group scale={fit}>
          <ParticleTrail kind={skin.trail % 4} />
        </group>
      ) : null}
      <primitive
        object={scene}
        scale={AGENT_SCALE}
        onClick={(event: ThreeEvent<MouseEvent>) => {
          event.stopPropagation();
          onFollow(index);
        }}
      />
      <group ref={tag} position={[0, 5.2, 0]}>
        <Html center sprite distanceFactor={12} zIndexRange={[8, 0]} style={{ pointerEvents: "none" }}>
          <div className={audit ? "name-tag audit" : "name-tag"}>
            <span className="live-dot" aria-hidden="true" />
            {agent.name}
          </div>
        </Html>
      </group>
    </group>
  );
}

function Grade({ shot }: { shot: "interior" | "hero" | "overview" }) {
  const gl = useThree((state) => state.gl);
  const scene = useThree((state) => state.scene);
  const camera = useThree((state) => state.camera);
  const size = useThree((state) => state.size);
  const shotKind = useRef(shot);
  shotKind.current = shot;
  const grade = useMemo(() => {
    const composer = new EffectComposer(gl, { multisampling: 0, frameBufferType: THREE.HalfFloatType });
    const normal = new NormalPass(scene, camera, { resolutionScale: 0.5 });
    const dof = new DepthOfFieldEffect(camera, { focusDistance: 18, focusRange: 8, bokehScale: 0.2, resolutionScale: 0.5 });
    const ssao = new SSAOEffect(camera, normal.texture, {
      samples: 9,
      rings: 4,
      intensity: 0.12,
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
        new BloomEffect({ intensity: 0.42, luminanceThreshold: 0.7, luminanceSmoothing: 0.18, mipmapBlur: true }),
        new ToneMappingEffect({ mode: ToneMappingMode.ACES_FILMIC }),
      ),
    );
    return { composer, dof };
  }, [camera, gl, scene]);

  useLayoutEffect(() => {
    grade.composer.setSize(size.width, size.height);
  }, [grade, size.height, size.width]);

  useLayoutEffect(
    () => () => {
      grade.composer.dispose();
    },
    [grade],
  );

  useFrame((_, delta) => {
    if (!grade.dof.target) grade.dof.target = new THREE.Vector3();
    camera.getWorldDirection(aim);
    const kind = shotKind.current;
    const focusDistance = kind === "interior" ? 5.4 : kind === "hero" ? 26 : 48;
    grade.dof.target.copy(camera.position).addScaledVector(aim, focusDistance);
    grade.dof.bokehScale = kind === "overview" ? 0.015 : 0;
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
  frameMode,
  lineup,
  street,
  poster,
  lobby,
}: {
  mode: "orbit" | "cutscene" | "play";
  beat: number;
  focusId: string | null;
  followIndex: number | null;
  paused: boolean;
  frameMode: "in" | "out";
  lineup: boolean;
  street: boolean;
  poster: string | null;
  lobby: boolean;
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
    const driving = mode === "cutscene" || lineup || street || Boolean(poster) || (mode === "play" && (followIndex !== null || Boolean(focusId)));
    if (wasDriving.current && !driving) homing.current = true;
    wasDriving.current = driving;
    rig.autoRotate = false;
    const applyFov = (fov: number) => {
      if (Math.abs(camera.fov - fov) < 0.05) return;
      camera.fov = fov;
      camera.updateProjectionMatrix();
    };
    if (mode === "orbit") {
      applyFov(lobby ? 46 : 48);
      rig.enabled = false;
      const mobileLobby = lobby && camera.aspect < 0.7;
      const baseEye = mobileLobby ? titleEyeMobile : titleEye;
      const baseAim = mobileLobby ? titleAimMobile : titleAim;
      if (lobby) {
        const t = state.clock.elapsedTime;
        const yaw = reduce.current ? 0 : Math.sin(t * 0.16) * 0.08;
        const bob = reduce.current ? 0 : Math.sin(t * 0.11) * 0.35;
        driftOffset.copy(baseEye).sub(baseAim);
        driftOffset.y = 0;
        driftOffset.applyAxisAngle(driftUp, yaw);
        eye.copy(baseAim).add(driftOffset);
        eye.y = baseEye.y + bob;
        aim.copy(baseAim);
      } else {
        aim.copy(overviewAim);
        eye.copy(overviewEye);
      }
      const blend = reduce.current || lobby ? 1 : 1 - Math.exp(-delta * 2);
      rig.target.lerp(aim, blend);
      camera.position.lerp(eye, blend);
      camera.lookAt(rig.target);
      return;
    }
    if (!driving && homing.current) {
      rig.enabled = false;
      applyFov(48);
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
      applyFov(beat < 4 ? 64 : 32);
      drive = true;
    } else if (poster === "front6") {
      eye.set(0, 3.42 * fit, 17);
      aim.set(0, 3.32 * fit, 0);
      applyFov(56);
      drive = true;
    } else if (poster === "q34") {
      eye.set(5.4, 3.55 * fit, 15.5);
      aim.set(0, 3.2 * fit, 0);
      applyFov(54);
      drive = true;
    } else if (poster === "expressions") {
      eye.set(0, 3.5 * fit, 16.5);
      aim.set(0, 3.3 * fit, 0);
      applyFov(46);
      drive = true;
    } else if (poster === "eyes") {
      eye.set(0, 7.2 * fit, 14);
      aim.set(0, 2.6 * fit, -0.8);
      applyFov(42);
      drive = true;
    } else if (poster === "gear") {
      eye.set(9, 4.4 * fit, 18);
      aim.set(0, 2.2 * fit, 0);
      applyFov(42);
      drive = true;
    } else if (poster === "face") {
      eye.set(FACE_SHOT.eye[0], FACE_SHOT.eye[1], FACE_SHOT.eye[2]);
      aim.set(FACE_SHOT.aim[0], FACE_SHOT.aim[1], FACE_SHOT.aim[2]);
      applyFov(FACE_SHOT.fov ?? 28);
      drive = true;
    } else if (poster === "walk") {
      eye.set(WALK_SHOT.eye[0], WALK_SHOT.eye[1], WALK_SHOT.eye[2]);
      aim.set(WALK_SHOT.aim[0], WALK_SHOT.aim[1], WALK_SHOT.aim[2]);
      applyFov(WALK_SHOT.fov ?? 42);
      drive = true;
    } else if (poster === "trails") {
      eye.set(0, 3.5 * fit, 16);
      aim.set(0, 2.3 * fit, 0);
      applyFov(42);
      drive = true;
    } else if (poster === "skins") {
      eye.set(0, 16 * fit, 26);
      aim.set(0, 2 * fit, 0);
      applyFov(48);
      drive = true;
    } else if (lineup) {
      eye.set(LINEUP_SHOT.eye[0], LINEUP_SHOT.eye[1], LINEUP_SHOT.eye[2]);
      aim.set(LINEUP_SHOT.aim[0], LINEUP_SHOT.aim[1], LINEUP_SHOT.aim[2]);
      applyFov(LINEUP_SHOT.fov ?? 50);
      drive = true;
    } else if (street) {
      eye.set(STREET_SHOT.eye[0], STREET_SHOT.eye[1], STREET_SHOT.eye[2]);
      aim.set(STREET_SHOT.aim[0], STREET_SHOT.aim[1], STREET_SHOT.aim[2]);
      applyFov(STREET_SHOT.fov ?? 52);
      drive = true;
    } else if (followIndex !== null) {
      applyFov(32);
      const pose = placeAgent(followIndex, state.clock.elapsedTime, paused);
      const aheadX = Math.sin(pose.rot);
      const aheadZ = Math.cos(pose.rot);
      aim.set(pose.x, 2.4 * fit, pose.z);
      eye.set(pose.x - aheadX * 9, 6.2 * fit, pose.z - aheadZ * 9);
      drive = true;
    } else if (focusId) {
      const room = DISTRICTS.find((district) => district.id === focusId);
      if (room) {
        const shot = (frameMode === "out" ? EXTERIOR_SHOTS : INTERIOR_SHOTS)[room.id];
        if (shot) {
          eye.set(shot.eye[0], shot.eye[1], shot.eye[2]);
          aim.set(shot.aim[0], shot.aim[1], shot.aim[2]);
          applyFov(shot.fov ?? 32);
        } else {
          eye.set(room.x, 2.6, room.z + 8);
          aim.set(room.x, 1.8, room.z);
        }
        drive = true;
      }
    }
    if (!drive) return;
    const blend = reduce.current || camera.position.distanceTo(eye) > 18 ? 1 : 1 - Math.exp(-delta * 6);
    if (blend === 1) {
      rig.target.copy(aim);
      camera.position.copy(eye);
    } else {
      rig.target.lerp(aim, blend);
      camera.position.lerp(eye, blend);
    }
    camera.lookAt(aim);
  });
  return null;
}

function ImpostorCrowd({ count, skip, paused }: { count: number; skip: number; paused: boolean }) {
  const mesh = useRef<THREE.InstancedMesh>(null);
  const camera = useThree((state) => state.camera);
  const slots = Math.max(0, count - skip);
  useLayoutEffect(() => {
    const field = mesh.current;
    if (!field) return;
    for (let index = 0; index < slots; index += 1) {
      impostorColor.set(PAINT_COLORS[(index + skip) % PAINT_COLORS.length]);
      field.setColorAt(index, impostorColor);
    }
    if (field.instanceColor) field.instanceColor.needsUpdate = true;
  }, [skip, slots]);
  useFrame((state) => {
    const field = mesh.current;
    if (!field) return;
    const t = state.clock.elapsedTime;
    for (let index = skip; index < count; index += 1) {
      const pose = placeAgent(index, t, paused);
      const dx = pose.x - camera.position.x;
      const dy = pose.y - camera.position.y;
      const dz = pose.z - camera.position.z;
      const dist = Math.sqrt(dx * dx + dy * dy + dz * dz);
      projected.set(pose.x, AGENT_HEAD, pose.z).project(camera);
      const outside = projected.z < 0 || projected.z > 1 || Math.abs(projected.x) > 1.15 || Math.abs(projected.y) > 1.15;
      const hidden = dist > CULL_DISTANCE || (outside && dist > 18);
      impostorDummy.position.set(pose.x, 0, pose.z);
      impostorDummy.rotation.set(0, pose.rot, 0);
      impostorDummy.scale.setScalar(hidden ? 0 : AGENT_SCALE);
      impostorDummy.updateMatrix();
      field.setMatrixAt(index - skip, impostorDummy.matrix);
    }
    field.instanceMatrix.needsUpdate = true;
  });
  if (slots === 0) return null;
  return (
    <instancedMesh ref={mesh} args={[undefined, undefined, slots]} frustumCulled={false} castShadow={false} receiveShadow>
      <capsuleGeometry args={[0.16, 0.62, 2, 6]} />
      <meshStandardMaterial roughness={0.55} metalness={0.15} />
    </instancedMesh>
  );
}

function FrameStats({ count }: { count: number }) {
  const gl = useThree((state) => state.gl);
  const bucket = useRef({ frames: 0, time: 0, calls: 0 });
  useLayoutEffect(() => {
    gl.info.autoReset = false;
    const node = document.createElement("p");
    node.id = "stress-readout";
    node.className = "stress-readout";
    node.textContent = `${count} agents`;
    document.body.appendChild(node);
    return () => {
      gl.info.autoReset = true;
      node.remove();
    };
  }, [count, gl]);
  useFrame((_, delta) => {
    const run = bucket.current;
    run.calls = gl.info.render.calls;
    gl.info.reset();
    run.frames += 1;
    run.time += delta;
    if (run.time < 0.5) return;
    const node = document.getElementById("stress-readout");
    if (node) {
      const fps = run.frames / run.time;
      node.textContent = `${count} agents · ${fps.toFixed(1)} fps · ${run.calls} draws · dpr ${gl.getPixelRatio().toFixed(2)}`;
    }
    run.frames = 0;
    run.time = 0;
  }, 2);
  return null;
}

export default function CityMap({
  paused,
  audit,
  mode,
  beat,
  focusId,
  followIndex,
  frameMode,
  lineup,
  street,
  poster,
  lobby = false,
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
  frameMode: "in" | "out";
  lineup: boolean;
  street: boolean;
  poster: string | null;
  lobby?: boolean;
  onPick: (district: District) => void;
  onFollow: (index: number) => void;
  onUnavailable: () => void;
}) {
  const [dpr, setDpr] = useState<number | [number, number]>([1, 1.5]);
  const [crowd] = useState(readCrowd);
  const crowdOn = crowd.measure && !poster && !lineup && !street;
  const fullSlots = crowdOn ? (crowd.full ? crowd.count : Math.min(crowd.count, FULL_CAP)) : 0;
  const shotId = mode === "cutscene" ? BEAT_IDS[Math.min(beat, BEAT_IDS.length - 1)] : frameMode === "in" ? focusId : null;
  const openInterior = shotId && INTERIOR_IDS.has(shotId) ? shotId : null;

  return (
    <Canvas
      className="city-canvas"
      dpr={dpr}
      shadows
      camera={{ position: [overviewEye.x, overviewEye.y, overviewEye.z], fov: 42, near: 0.35, far: 2500 }}
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
      <color attach="background" args={["#070a1a"]} />
      <World />
      <Nature />
      <Kits onPick={onPick} focusId={focusId} frameMode={frameMode} beatId={openInterior} />
      <Interiors focusId={openInterior} frameMode={openInterior ? "in" : frameMode} />
      <group position={[-42, 6.4, 33]}>
        <mesh>
          <boxGeometry args={[7.2, 1.35, 0.16]} />
          <meshStandardMaterial color="#e2b340" metalness={0.6} roughness={0.32} emissive="#ffc14a" emissiveIntensity={1.4} />
        </mesh>
        <Text position={[0, 0, 0.1]} font={DISPLAY_FONT} fontSize={0.48} color="#2a1c08" anchorX="center" anchorY="middle">
          VAULT
        </Text>
      </group>
      <Smoke position={[-34, 13.4, -26]} />
      {DISTRICTS.filter((district) => district.status === "soon").map((district) => (
        <group key={district.id} position={[district.x, 5.2, district.z + 9]}>
          <mesh>
            <boxGeometry args={[8.5, 2.4, 0.25]} />
            <meshStandardMaterial color="#f4e7c4" roughness={0.6} emissive="#e8c48a" emissiveIntensity={0.45} />
          </mesh>
          <Text
            position={[0, 0, 0.16]}
            font={DISPLAY_FONT}
            fontSize={0.62}
            color="#1c140c"
            anchorX="center"
            anchorY="middle"
          >
            COMING SOON
          </Text>
        </group>
      ))}
      {DISTRICTS.filter((district) => district.status === "live").flatMap((district) =>
        [
          [district.x - 2.6, 4.8, district.z + 7.8],
          [district.x + 2.4, 6.6, district.z + 7.8],
          [district.x, 8.8, district.z + 7.5],
        ].map((position) => (
          <mesh key={`${district.id}-${position[1]}`} position={position as [number, number, number]}>
            <planeGeometry args={[1.5, 1.15]} />
            <meshStandardMaterial color="#ffd7a2" emissive="#ffc56a" emissiveIntensity={3.4} roughness={0.35} />
          </mesh>
        )),
      )}
      {DISTRICTS.filter((district) => district.status === "live").map((district) => (
        <pointLight key={`${district.id}-window`} position={[district.x, 6.2, district.z + 8]} color="#ffb060" intensity={18} distance={22} decay={2} />
      ))}
      <mesh position={[12, 16.2, 40]}>
        <sphereGeometry args={[0.32, 10, 8]} />
        <meshStandardMaterial color="#ff3030" emissive="#ff2020" emissiveIntensity={3.2} />
      </mesh>
      <pointLight position={[12, 15.4, 40]} color="#ff4040" intensity={5} distance={16} decay={2} />
      <mesh position={[-36.2, 3.4, -16.5]}>
        <boxGeometry args={[1.6, 1.2, 0.18]} />
        <meshStandardMaterial color="#ff6a1a" emissive="#ff5a12" emissiveIntensity={2.4} />
      </mesh>
      <PerformanceMonitor flipflops={4} onDecline={() => setDpr(1)} onIncline={() => setDpr([1, 1.5])} />
      <pointLight position={[-42, 2.4, 18]} intensity={6} color="#ffb060" distance={14} decay={2} />
      <pointLight position={[46, 2.4, 14]} intensity={5} color="#ffd2a8" distance={14} decay={2} />
      <pointLight position={[12, 2.4, 40]} intensity={4} color="#9fd0ff" distance={12} decay={2} />
      <pointLight position={[-38, 2.4, -24]} intensity={5} color="#ff6a1a" distance={12} decay={2} />
      {lineup ? (
        <mesh position={[0, 4.2, -3.5]}>
          <planeGeometry args={[52, 16]} />
          <meshBasicMaterial color="#101018" />
        </mesh>
      ) : null}
      <Station onPick={onPick} focusId={focusId} beatPrefix={mode === "cutscene" ? (BEAT_PREFIX[beat] ?? null) : null} />
      {crowdOn
        ? Array.from({ length: fullSlots }, (_, index) => (
            <AgentMesh
              key={`crowd-${index}`}
              agent={CITY_AGENTS[index % CITY_AGENTS.length]}
              index={index}
              paused={paused}
              audit={audit}
              focusId={focusId}
              lineup={false}
              poster={null}
              onFollow={onFollow}
              rim={crowd.count <= FULL_CAP}
            />
          ))
        : (poster === "skins" ? Array.from({ length: 12 }, (_, index) => CITY_AGENTS[index % CITY_AGENTS.length]) : CITY_AGENTS).map((agent, index) => (
            <AgentMesh key={`${poster ?? "city"}-${index}`} agent={agent} index={index} paused={paused || lineup || Boolean(poster)} audit={audit} focusId={focusId} lineup={lineup} poster={poster} onFollow={onFollow} />
          ))}
      {crowdOn && !crowd.full && crowd.count > FULL_CAP ? <ImpostorCrowd count={crowd.count} skip={FULL_CAP} paused={paused} /> : null}
      {crowd.measure ? <FrameStats count={crowd.count} /> : null}
      <Ears paused={paused} />
      <CameraDriver mode={mode} beat={beat} focusId={focusId} followIndex={followIndex} paused={paused} frameMode={frameMode} lineup={lineup} street={street} poster={poster} lobby={lobby} />
      <Grade shot={openInterior ? "interior" : focusId || mode === "cutscene" ? "hero" : "overview"} />
      <OrbitControls
        makeDefault
        enabled={!focusId && followIndex === null && mode !== "cutscene" && !lineup && !street}
        enablePan={false}
        autoRotate={false}
        target={[overviewAim.x, overviewAim.y, overviewAim.z]}
        minPolarAngle={0.35}
        maxPolarAngle={1.5}
        minDistance={8}
        maxDistance={360}
      />
    </Canvas>
  );
}
