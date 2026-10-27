"use client";

import { useGLTF } from "@react-three/drei";
import { Suspense, useMemo } from "react";
import * as THREE from "three";

const ROOMS: Record<string, { url: string; position: [number, number, number]; wall: string; floor: string }> = {
  "vault-quarter": { url: "/models/interior-vault.glb", position: [-42, 0, 18], wall: "#8d6438", floor: "#6a4a2e" },
  "trading-village": { url: "/models/interior-market.glb", position: [46, 0, 14], wall: "#8a3e1c", floor: "#5c3018" },
  "war-room": { url: "/models/interior-war.glb", position: [12, 0, 40], wall: "#3a4654", floor: "#242a30" },
  "skin-forge": { url: "/models/interior-forge.glb", position: [-38, 0, -24], wall: "#7a2e1c", floor: "#542418" },
  "cup-stadium": { url: "/models/interior-stadium.glb", position: [-8, 0, -34], wall: "#a89068", floor: "#6e5838" },
};

function sourceName(node: THREE.Object3D): string {
  let current: THREE.Object3D | null = node;
  while (current) {
    const stored = current.userData.name;
    if (typeof stored === "string" && stored.includes(".")) return stored;
    current = current.parent;
  }
  return node.name;
}

function paintFinish(mesh: THREE.Mesh, color: string, roughness: number) {
  const source = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
  const copies = source.map((mat) => {
    const copy = mat.clone();
    const standard = copy as THREE.MeshStandardMaterial;
    if (standard.isMeshStandardMaterial) {
      standard.color.set(color);
      standard.roughness = roughness;
      standard.metalness = 0;
      standard.envMapIntensity = 0.06;
      standard.map = null;
    }
    return copy;
  });
  mesh.material = Array.isArray(mesh.material) ? copies : copies[0];
}

function Room({ url, wall, floor }: { url: string; wall: string; floor: string }) {
  const gltf = useGLTF(url, false, true);
  const scene = useMemo(() => {
    const cloned = gltf.scene.clone(true);
    cloned.traverse((node) => {
      const mesh = node as THREE.Mesh;
      const label = sourceName(node);
      if (label.includes("wall.e")) node.visible = false;
      if (!mesh.isMesh) return;
      mesh.castShadow = true;
      mesh.receiveShadow = true;
      mesh.frustumCulled = false;
      if (label.includes("wall")) paintFinish(mesh, wall, 0.9);
      else if (label.includes("floor")) paintFinish(mesh, floor, 0.94);
      else if (mesh.material) {
        const source = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
        source.forEach((mat) => {
          const standard = mat as THREE.MeshStandardMaterial;
          if (standard.isMeshStandardMaterial) standard.envMapIntensity = 0.12;
        });
      }
    });
    return cloned;
  }, [gltf.scene, wall, floor]);
  return <primitive object={scene} />;
}

export function Interiors({ focusId, frameMode }: { focusId: string | null; frameMode: "in" | "out" }) {
  if (frameMode !== "in" || !focusId || !ROOMS[focusId]) return null;
  const room = ROOMS[focusId];
  return (
    <Suspense fallback={null}>
      <group position={room.position}>
        <Room key={room.url} url={room.url} wall={room.wall} floor={room.floor} />
      </group>
    </Suspense>
  );
}
