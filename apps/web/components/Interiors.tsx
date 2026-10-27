"use client";

import { Text, useGLTF } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";
import { Suspense, useLayoutEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import { DISPLAY_FONT } from "@/lib/fonts";
import { tapeLines } from "@/lib/tape";

const ROOMS: Record<string, { url: string; position: [number, number, number] }> = {
  "vault-quarter": { url: "/models/interior-vault.glb", position: [-42, 0, 18] },
  "trading-village": { url: "/models/interior-market.glb", position: [46, 0, 14] },
  "war-room": { url: "/models/interior-war.glb", position: [12, 0, 40] },
  "skin-forge": { url: "/models/interior-forge.glb", position: [-38, 0, -24] },
  "cup-stadium": { url: "/models/interior-stadium.glb", position: [-8, 0, -42] },
};

const LAMPS: Array<[number, number, number, string]> = [
  [0, 3.15, -1.2, "#ff9a3c"],
  [-3.4, 2.5, 0.8, "#ffb15a"],
  [3.2, 2.3, -0.4, "#ff8a32"],
  [0.4, 2.1, 2.6, "#ffc48a"],
];

function RoomMesh({ url }: { url: string }) {
  const gltf = useGLTF(url, false, true);
  const scene = useMemo(() => {
    const cloned = gltf.scene.clone(true);
    cloned.traverse((node) => {
      const mesh = node as THREE.Mesh;
      if (!mesh.isMesh) return;
      mesh.castShadow = true;
      mesh.receiveShadow = true;
      mesh.frustumCulled = false;
      const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
      materials.forEach((material) => {
        const standard = material as THREE.MeshStandardMaterial;
        if (standard.emissiveIntensity > 1.05) standard.emissiveIntensity = 1.05;
      });
    });
    return cloned;
  }, [gltf.scene]);
  return <primitive object={scene} />;
}

function Tape({ position }: { position: [number, number, number] }) {
  const [lines, setLines] = useState(() => tapeLines());
  const minute = useRef(Math.floor(Date.now() / 60000));
  useFrame(() => {
    const next = Math.floor(Date.now() / 60000);
    if (next === minute.current) return;
    minute.current = next;
    setLines(tapeLines());
  });
  return (
    <group position={position}>
      {lines.map((line, index) => (
        <Text
          key={line}
          position={[0, 0.42 - index * 0.38, 0.08]}
          font={DISPLAY_FONT}
          fontSize={0.22}
          color="#2fb86a"
          anchorX="center"
          anchorY="middle"
        >
          {line}
        </Text>
      ))}
    </group>
  );
}

function SeatCrowd() {
  const bodies = useRef<THREE.InstancedMesh>(null);
  const heads = useRef<THREE.InstancedMesh>(null);
  const spots = useMemo(() => {
    const list: Array<[number, number, number]> = [];
    for (let row = 0; row < 4; row += 1) {
      for (let seat = 0; seat < 10; seat += 1) {
        list.push([-4.6 + seat * 1.02, 0.55 + row * 0.28, 2.55 + row * 0.72]);
      }
    }
    return list;
  }, []);
  useLayoutEffect(() => {
    const dummy = new THREE.Object3D();
    spots.forEach(([x, y, z], index) => {
      dummy.position.set(x, y, z);
      dummy.rotation.set(0, 0, 0);
      dummy.scale.set(1, 1, 1);
      dummy.updateMatrix();
      bodies.current?.setMatrixAt(index, dummy.matrix);
      dummy.position.set(x, y + 0.34, z);
      dummy.updateMatrix();
      heads.current?.setMatrixAt(index, dummy.matrix);
    });
    if (bodies.current) bodies.current.instanceMatrix.needsUpdate = true;
    if (heads.current) heads.current.instanceMatrix.needsUpdate = true;
  }, [spots]);
  return (
    <>
      <instancedMesh ref={bodies} args={[undefined, undefined, spots.length]} frustumCulled={false}>
        <capsuleGeometry args={[0.12, 0.22, 2, 6]} />
        <meshStandardMaterial color="#3a2a24" roughness={0.72} />
      </instancedMesh>
      <instancedMesh ref={heads} args={[undefined, undefined, spots.length]} frustumCulled={false}>
        <sphereGeometry args={[0.1, 8, 6]} />
        <meshStandardMaterial color="#c4a484" roughness={0.6} />
      </instancedMesh>
    </>
  );
}

function RoomCopy({ id }: { id: string }) {
  if (id === "vault-quarter") {
    return (
      <>
        <Text position={[0, 0.16, 5.15]} rotation={[-Math.PI / 2, 0, 0]} font={DISPLAY_FONT} fontSize={0.22} color="#ffb15a" anchorX="center">
          ENTRY
        </Text>
        <Text position={[5.35, 0.7, -1.2]} font={DISPLAY_FONT} fontSize={0.18} color="#ff5a4a" anchorX="center">
          EXIT
        </Text>
        <Text position={[-2.2, 2.75, -5.55]} font={DISPLAY_FONT} fontSize={0.16} color="#d4d2cc" anchorX="center">
          NEXT 18:00Z
        </Text>
        <Text position={[-2.2, 2.35, -5.55]} font={DISPLAY_FONT} fontSize={0.16} color="#d4d2cc" anchorX="center">
          MASON 12
        </Text>
      </>
    );
  }
  if (id === "trading-village") {
    return (
      <>
        <Tape position={[0, 2.35, -5.72]} />
        <Text position={[-2.6, 2.55, -5.62]} font={DISPLAY_FONT} fontSize={0.2} color="#d4d2cc" anchorX="center">
          1 TAPE
        </Text>
        <Text position={[-2.6, 2.15, -5.62]} font={DISPLAY_FONT} fontSize={0.2} color="#d4d2cc" anchorX="center">
          2 MASON
        </Text>
        <Text position={[-2.6, 1.75, -5.62]} font={DISPLAY_FONT} fontSize={0.2} color="#d4d2cc" anchorX="center">
          3 SCOUT
        </Text>
        <Text position={[3.6, 2.05, -5.6]} font={DISPLAY_FONT} fontSize={0.12} color="#d4d2cc" anchorX="center">
          PERP NEAR
        </Text>
        <Text position={[3.6, 1.7, -5.6]} font={DISPLAY_FONT} fontSize={0.12} color="#d4d2cc" anchorX="center">
          STK IRON
        </Text>
        <Text position={[3.6, 1.35, -5.6]} font={DISPLAY_FONT} fontSize={0.12} color="#2fb86a" anchorX="center">
          PNL
        </Text>
      </>
    );
  }
  if (id === "war-room") {
    return (
      <>
        <Text position={[-4.2, 2.5, -5.5]} font={DISPLAY_FONT} fontSize={0.16} color="#d4d2cc" anchorX="center">
          CONTRACT
        </Text>
        <Text position={[0, 2.5, -5.5]} font={DISPLAY_FONT} fontSize={0.16} color="#d4d2cc" anchorX="center">
          WEB
        </Text>
        <Text position={[4.2, 2.5, -5.5]} font={DISPLAY_FONT} fontSize={0.16} color="#d4d2cc" anchorX="center">
          CRYPTO
        </Text>
      </>
    );
  }
  if (id === "skin-forge") {
    return (
      <>
        <Text position={[0, 2.7, -5.55]} font={DISPLAY_FONT} fontSize={0.18} color="#d4d2cc" anchorX="center">
          SCHEDULE
        </Text>
        <Text position={[0, 2.25, -5.55]} font={DISPLAY_FONT} fontSize={0.16} color="#d4d2cc" anchorX="center">
          MASON
        </Text>
        <Text position={[-6.15, 2.1, -2.4]} rotation={[0, Math.PI / 2, 0]} font={DISPLAY_FONT} fontSize={0.16} color="#d4d2cc" anchorX="center">
          VAULT
        </Text>
        <Text position={[-6.15, 2.1, 1.6]} rotation={[0, Math.PI / 2, 0]} font={DISPLAY_FONT} fontSize={0.16} color="#d4d2cc" anchorX="center">
          FLOOR
        </Text>
        <Text position={[6.15, 2.1, -0.6]} rotation={[0, -Math.PI / 2, 0]} font={DISPLAY_FONT} fontSize={0.16} color="#d4d2cc" anchorX="center">
          WAR
        </Text>
        <Text position={[3.4, 2.35, -5.58]} font={DISPLAY_FONT} fontSize={0.14} color="#e2b340" anchorX="center">
          SKINS
        </Text>
      </>
    );
  }
  return (
    <Text position={[0, 2.7, -5.68]} font={DISPLAY_FONT} fontSize={0.34} color="#2fb86a" anchorX="center">
      CUP  2-1
    </Text>
  );
}

export function Interiors({ focusId, frameMode }: { focusId: string | null; frameMode: "in" | "out" }) {
  if (frameMode !== "in" || !focusId || !ROOMS[focusId]) return null;
  const room = ROOMS[focusId];
  return (
    <Suspense fallback={null}>
      <group position={room.position}>
        <RoomMesh key={room.url} url={room.url} />
        {LAMPS.map(([x, y, z, color], index) => (
          <pointLight key={index} position={[x, y, z]} color={color} intensity={110} distance={12} decay={2} />
        ))}
        <RoomCopy id={focusId} />
        {focusId === "cup-stadium" ? <SeatCrowd /> : null}
      </group>
    </Suspense>
  );
}

for (const room of Object.values(ROOMS)) useGLTF.preload(room.url, false, true);
