"use client";

import { useGLTF } from "@react-three/drei";
import { Suspense, useMemo } from "react";
import * as THREE from "three";

const ROOMS: Record<string, { url: string; position: [number, number, number] }> = {
  "vault-quarter": { url: "/models/interior-vault.glb", position: [-42, 0, 18] },
  "trading-village": { url: "/models/interior-market.glb", position: [46, 0, 14] },
  "war-room": { url: "/models/interior-war.glb", position: [12, 0, 40] },
  "skin-forge": { url: "/models/interior-forge.glb", position: [-38, 0, -24] },
  "cup-stadium": { url: "/models/interior-stadium.glb", position: [-8, 0, -34] },
};

function Room({ url }: { url: string }) {
  const gltf = useGLTF(url, false, true);
  const scene = useMemo(() => {
    const cloned = gltf.scene.clone(true);
    cloned.traverse((node) => {
      const mesh = node as THREE.Mesh;
      if (!mesh.isMesh) return;
      mesh.castShadow = true;
      mesh.receiveShadow = true;
      mesh.frustumCulled = false;
    });
    return cloned;
  }, [gltf.scene]);
  return <primitive object={scene} />;
}

export function Interiors({ focusId, frameMode }: { focusId: string | null; frameMode: "in" | "out" }) {
  if (frameMode !== "in" || !focusId || !ROOMS[focusId]) return null;
  const room = ROOMS[focusId];
  return (
    <Suspense fallback={null}>
      <group position={room.position}>
        <Room key={room.url} url={room.url} />
      </group>
    </Suspense>
  );
}
