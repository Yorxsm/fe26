"use client";

import { useGLTF } from "@react-three/drei";
import { useFrame, useThree } from "@react-three/fiber";
import { useMemo, useRef } from "react";
import * as THREE from "three";
import { DISTRICTS } from "@/lib/city";

useGLTF.preload("/models/nature.glb", false, true);

const RING: Array<[number, number]> = [
  [-54, -2],
  [-38, -24],
  [-8, -42],
  [50, -28],
  [20, -10],
  [46, 14],
  [12, 40],
  [-42, 18],
];

const TREES = ["pine.a", "pine.b", "oak.a", "oak.b", "birch.a", "birch.b"];

type Spot = { name: string; x: number; z: number; rot: number; scale: number };

function blocksFront(x: number, z: number) {
  return DISTRICTS.some((district) => Math.abs(x - district.x) < 18 && z > district.z - 2 && z < district.z + 20);
}

function nearBuilding(x: number, z: number, radius: number) {
  return DISTRICTS.some((district) => {
    const dx = district.x - x;
    const dz = district.z - z;
    return dx * dx + dz * dz < radius * radius;
  });
}

function nearRoad(x: number, z: number, radius: number) {
  for (let index = 0; index < RING.length; index += 1) {
    const [ax, az] = RING[index];
    const [bx, bz] = RING[(index + 1) % RING.length];
    const dx = bx - ax;
    const dz = bz - az;
    const len2 = dx * dx + dz * dz || 1;
    const t = Math.max(0, Math.min(1, ((x - ax) * dx + (z - az) * dz) / len2));
    const px = ax + dx * t;
    const pz = az + dz * t;
    if ((x - px) ** 2 + (z - pz) ** 2 < radius * radius) return true;
  }
  return false;
}

function spots(): Spot[] {
  const placed: Spot[] = [];
  let n = 0;
  for (let index = 0; index < RING.length; index += 1) {
    const [ax, az] = RING[index];
    const [bx, bz] = RING[(index + 1) % RING.length];
    const dx = bx - ax;
    const dz = bz - az;
    const length = Math.hypot(dx, dz) || 1;
    const ux = dx / length;
    const uz = dz / length;
    const px = -uz;
    const pz = ux;
    for (let dist = 8; dist < length - 6; dist += 11) {
      for (const side of [-1, 1]) {
        const x = ax + ux * dist + px * side * 16;
        const z = az + uz * dist + pz * side * 16;
        if (Math.abs(x) > 100 || Math.abs(z) > 100 || nearBuilding(x, z, 20) || blocksFront(x, z)) continue;
        const name = TREES[n % TREES.length];
        placed.push({ name, x, z, rot: n * 0.7, scale: 0.85 + (n % 5) * 0.07 });
        n += 1;
        placed.push({
          name: n % 2 === 0 ? "bush.a" : "bush.b",
          x: x + px * side * 2.4,
          z: z + pz * side * 2.4,
          rot: n,
          scale: 0.9 + (n % 3) * 0.15,
        });
      }
    }
  }
  for (let i = 0; i < 48; i += 1) {
    const angle = i * 2.399;
    const radius = 36 + (i * 7) % 52;
    const x = Math.cos(angle) * radius;
    const z = Math.sin(angle) * radius;
    if (nearBuilding(x, z, 20) || nearRoad(x, z, 11) || blocksFront(x, z)) continue;
    placed.push({ name: TREES[i % TREES.length], x, z, rot: angle, scale: 0.9 + (i % 4) * 0.08 });
    if (i % 2 === 0) {
      placed.push({ name: "rock.a", x: x + 3, z: z - 2, rot: angle, scale: 0.8 + (i % 3) * 0.3 });
    }
  }
  for (let i = 0; i < 18; i += 1) {
    const angle = i * 0.7;
    const x = 8 + Math.cos(angle) * (7 + (i % 3));
    const z = 6 + Math.sin(angle) * (6 + (i % 2));
    placed.push({ name: i % 3 === 0 ? "rock.c" : "plant.a", x, z, rot: angle, scale: 1 });
    placed.push({ name: "flower.a", x: x + 1.4, z: z + 0.6, rot: angle, scale: 1.1 });
  }
  return placed;
}

const PLACED = spots();

function windMaterials(source: THREE.Material | THREE.Material[], amount: number, uTime: { value: number }) {
  const list = (Array.isArray(source) ? source : [source]).map((material) => material.clone()) as THREE.MeshStandardMaterial[];
  list.forEach((material) => {
    material.envMapIntensity = 0.04;
    if (amount < 0.1 && material.color) material.color.multiply(new THREE.Color("#4d6a58"));
    material.onBeforeCompile = (shader) => {
      shader.uniforms.uTime = uTime;
      shader.uniforms.uWind = { value: amount };
      shader.vertexShader = shader.vertexShader.replace(
        "#include <begin_vertex>",
        `#include <begin_vertex>
         #ifdef USE_INSTANCING
           vec3 windOrigin = vec3(instanceMatrix[3][0], instanceMatrix[3][1], instanceMatrix[3][2]);
         #else
           vec3 windOrigin = vec3(0.0);
         #endif
         float crown = smoothstep(0.15, 1.0, position.y);
         float gust = sin(uTime * 1.25 + windOrigin.x * 0.17 + windOrigin.z * 0.11);
         transformed.x += gust * uWind * crown;
         transformed.z += cos(uTime * 0.85 + windOrigin.z * 0.13) * uWind * 0.65 * crown;`,
      );
    };
  });
  return list.length === 1 ? list[0] : list;
}

function makeField(src: THREE.Mesh, items: Spot[], amount: number, uTime: { value: number }) {
  const mesh = new THREE.InstancedMesh(src.geometry, windMaterials(src.material, amount, uTime), Math.max(1, items.length));
  const dummy = new THREE.Object3D();
  items.forEach((item, index) => {
    dummy.position.set(item.x, 0, item.z);
    dummy.rotation.set(0, item.rot, 0);
    dummy.scale.setScalar(item.scale);
    dummy.updateMatrix();
    mesh.setMatrixAt(index, dummy.matrix);
  });
  mesh.instanceMatrix.needsUpdate = true;
  mesh.count = items.length;
  mesh.frustumCulled = false;
  mesh.userData.spots = items;
  mesh.userData.base = items.map((_, index) => {
    const matrix = new THREE.Matrix4();
    mesh.getMatrixAt(index, matrix);
    return matrix;
  });
  mesh.castShadow = amount > 0.05;
  mesh.receiveShadow = true;
  return mesh;
}

function GrassField({ source, uTime }: { source: THREE.Mesh; uTime: { value: number } }) {
  const mesh = useMemo(() => {
    const field = new THREE.InstancedMesh(source.geometry, windMaterials(source.material, 0.06, uTime), 1400);
    field.frustumCulled = false;
    field.receiveShadow = true;
    field.count = 1400;
    return field;
  }, [source, uTime]);
  const anchor = useRef(new THREE.Vector2(999, 999));
  const camera = useThree((state) => state.camera);
  useFrame(() => {
    const x = camera.position.x;
    const z = camera.position.z;
    if ((x - anchor.current.x) ** 2 + (z - anchor.current.y) ** 2 < 16) return;
    anchor.current.set(x, z);
    const dummy = new THREE.Object3D();
    for (let index = 0; index < 1400; index += 1) {
      const angle = index * 2.399;
      const radius = (index % 38) * 0.55;
      const gx = x + Math.cos(angle) * radius;
      const gz = z + Math.sin(angle) * radius;
      dummy.position.set(gx, 0, gz);
      dummy.rotation.set(0, angle, 0);
      const indoors = radius < 2.2 || nearBuilding(gx, gz, 9);
      dummy.scale.setScalar(indoors ? 0 : 0.85 + (index % 5) * 0.08);
      dummy.updateMatrix();
      mesh.setMatrixAt(index, dummy.matrix);
    }
    mesh.instanceMatrix.needsUpdate = true;
  });
  return <primitive object={mesh} />;
}

function blocksView(x: number, z: number, camera: THREE.Vector3, direction: THREE.Vector3) {
  const reach = 32;
  const vx = direction.x * reach;
  const vz = direction.z * reach;
  const len2 = vx * vx + vz * vz || 1;
  const t = Math.max(0, Math.min(1, ((x - camera.x) * vx + (z - camera.z) * vz) / len2));
  if (t < 0.08 || t > 0.9) return false;
  const px = camera.x + vx * t;
  const pz = camera.z + vz * t;
  return (x - px) ** 2 + (z - pz) ** 2 < 18;
}

export function Nature() {
  const gltf = useGLTF("/models/nature.glb", false, true);
  const camera = useThree((state) => state.camera);
  const view = useRef(new THREE.Vector3());
  const uTime = useMemo(() => ({ value: 0 }), []);
  const fields = useMemo(() => {
    const names = new Set(PLACED.map((spot) => spot.name));
    return [...names].map((name) => {
      const src = gltf.scene.getObjectByName(name) as THREE.Mesh | undefined;
      if (!src?.isMesh) return null;
      const amount = name.startsWith("rock") || name.startsWith("flower") ? 0 : name.startsWith("bush") ? 0.12 : 0.28;
      return makeField(src, PLACED.filter((spot) => spot.name === name), amount, uTime);
    });
  }, [gltf.scene, uTime]);
  const grass = gltf.scene.getObjectByName("grass.a") as THREE.Mesh | undefined;
  useFrame(({ clock }) => {
    uTime.value = clock.elapsedTime;
    camera.getWorldDirection(view.current);
    const hidden = new THREE.Matrix4().makeScale(0, 0, 0);
    fields.forEach((field) => {
      if (!field) return;
      const spots = field.userData.spots as Spot[] | undefined;
      const base = field.userData.base as THREE.Matrix4[] | undefined;
      if (!spots || !base) return;
      let changed = false;
      spots.forEach((spot, index) => {
        const tree = spot.name.startsWith("pine") || spot.name.startsWith("oak") || spot.name.startsWith("birch");
        const cull = tree && blocksView(spot.x, spot.z, camera.position, view.current);
        const next = cull ? hidden : base[index];
        field.setMatrixAt(index, next);
        changed = true;
      });
      if (changed) field.instanceMatrix.needsUpdate = true;
    });
  });
  return (
    <group>
      {fields.map((field, index) => (field ? <primitive key={index} object={field} /> : null))}
      {grass?.isMesh ? <GrassField source={grass} uTime={uTime} /> : null}
    </group>
  );
}
