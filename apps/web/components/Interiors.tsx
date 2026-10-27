"use client";

import { useGLTF } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";
import { Suspense, useLayoutEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import { CHALLENGES, countdownSecs, formatCountdown, heistRows, mockQuotes } from "@/lib/schedule";

const ROOMS: Record<string, { url: string; position: [number, number, number] }> = {
  "vault-quarter": { url: "/models/interior-vault.glb", position: [-42, 0, 18] },
  "trading-village": { url: "/models/interior-market.glb", position: [46, 0, 14] },
  "war-room": { url: "/models/interior-war.glb", position: [12, 0, 40] },
  "skin-forge": { url: "/models/interior-forge.glb", position: [-38, 0, -24] },
  "cup-stadium": { url: "/models/interior-stadium.glb", position: [-8, 0, -42] },
};

const LAMPS: Array<[number, number, number]> = [
  [-2.2, 3.2, -1.4],
  [2.2, 3.2, -1.4],
  [0, 3.15, 1.6],
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
        const name = mesh.name.toLowerCase();
        if (name.includes("laser")) standard.emissiveIntensity = 2.1;
        else if (name.includes("glow")) standard.emissiveIntensity = Math.min(standard.emissiveIntensity, 0.32);
        else standard.emissiveIntensity = Math.min(standard.emissiveIntensity, 0.05);
      });
    });
    return cloned;
  }, [gltf.scene]);
  return <primitive object={scene} />;
}

function quotedFamily(variable: string, fallback: string) {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(variable);
  const first = raw.split(",")[0]?.trim().replace(/^['"]|['"]$/g, "");
  return first || fallback;
}

function paintBoard(canvas: HTMLCanvasElement, lines: string[], compact: boolean) {
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const display = quotedFamily("--font-display", "sans-serif");
  const mono = quotedFamily("--font-mono", "monospace");
  const body = lines.slice(1, 6);
  const titleSize = compact ? 150 : 88;
  const bodySize = compact ? 140 : 68;
  const titleY = compact ? 70 : 44;
  const bodyTop = compact ? 320 : 164;
  const bodyGap = compact ? 200 : 78;
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = "#d4d2cc";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = "#c94a1a";
  ctx.fillRect(0, 0, canvas.width, compact ? 36 : 18);
  ctx.fillStyle = "#1f8a4e";
  ctx.fillRect(0, compact ? 36 : 18, compact ? 28 : 16, canvas.height);
  ctx.textBaseline = "top";
  ctx.fillStyle = "#141416";
  ctx.font = `700 ${titleSize}px "${display}", sans-serif`;
  ctx.fillText(lines[0] ?? "", 64, titleY);
  ctx.font = `500 ${bodySize}px "${mono}", monospace`;
  body.forEach((line, index) => {
    ctx.fillText(line, 64, bodyTop + index * bodyGap);
  });
}

function Screen({
  position,
  lines,
  size = [3.4, 1.92],
}: {
  position: [number, number, number];
  lines: string[];
  size?: [number, number];
}) {
  const key = lines.join("|");
  const [width, height] = size;
  const compact = height < 1.2;
  const map = useMemo(() => {
    const canvas = document.createElement("canvas");
    canvas.width = 1024;
    canvas.height = 576;
    const texture = new THREE.CanvasTexture(canvas);
    texture.colorSpace = THREE.SRGBColorSpace;
    texture.flipY = true;
    texture.anisotropy = 8;
    texture.needsUpdate = true;
    return texture;
  }, []);
  useLayoutEffect(() => {
    let dead = false;
    const draw = () => {
      paintBoard(map.image as HTMLCanvasElement, lines, compact);
      map.needsUpdate = true;
    };
    draw();
    const display = quotedFamily("--font-display", "sans-serif");
    const mono = quotedFamily("--font-mono", "monospace");
    const fonts = Promise.all([
      document.fonts.load(`700 ${compact ? 150 : 88}px "${display}"`),
      document.fonts.load(`500 ${compact ? 140 : 68}px "${mono}"`),
    ]).catch(() => undefined);
    const raced = Promise.race([fonts, new Promise((resolve) => window.setTimeout(resolve, 400))]);
    raced.then(() => {
      if (!dead) draw();
    });
    return () => {
      dead = true;
    };
    // key is the line text. Repaint when it changes, including after the font load.
  }, [key, map, compact]);
  return (
    <group position={position}>
      <mesh position={[0, 0, -0.06]} castShadow>
        <boxGeometry args={[width + 0.18, height + 0.18, 0.08]} />
        <meshStandardMaterial color="#2c2e32" metalness={0.78} roughness={0.38} />
      </mesh>
      <mesh position={[0, 0, 0.02]} renderOrder={2}>
        <planeGeometry args={[width, height]} />
        <meshBasicMaterial map={map} color="#ffffff" toneMapped={false} />
      </mesh>
    </group>
  );
}

function useClock() {
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));
  const last = useRef(now);
  useFrame(() => {
    const next = Math.floor(Date.now() / 1000);
    if (next - last.current < 15) return;
    last.current = next;
    setNow(next);
  });
  return now;
}

function RoomBoards({ id }: { id: string }) {
  const now = useClock();
  const countdown = formatCountdown(countdownSecs(now));
  const ranks = heistRows("season", now).slice(0, 5);
  const rankLines = ["SEASON", ...ranks.map((row, index) => `${index + 1}  ${row.name.toUpperCase()}  ${row.points}`)];
  const quotes = mockQuotes(now);
  if (id === "vault-quarter") {
    return (
      <>
        <Screen position={[-4.35, 2.2, -5.86]} lines={["SCHEDULE", `NEXT  ${countdown}`, "DAILY  18:00Z", "WEEK  MON", "SEASON 1"]} />
        <Screen position={[4.35, 2.2, -5.86]} lines={rankLines} />
      </>
    );
  }
  if (id === "trading-village") {
    return (
      <>
        <Screen
          position={[-3.3, 2.25, -5.82]}
          size={[5.2, 2.2]}
          lines={[
            "PERPS",
            ...quotes.map((row) => `${row.symbol}  ${(row.mark / 1_000_000).toFixed(2)}`),
          ]}
        />
        <Screen
          position={[4.15, 2.15, -5.82]}
          size={[3.2, 2.0]}
          lines={["PNL", ...quotes.map((row) => `${row.fundingBps} BPS  ${(row.pnlMicros / 1_000_000).toFixed(2)}`)]}
        />
      </>
    );
  }
  if (id === "war-room") {
    return (
      <>
        <Screen position={[-4.4, 1.15, -3.4]} size={[2.2, 0.7]} lines={["CONTRACT", `${CHALLENGES[0]?.points ?? 0}`]} />
        <Screen position={[0, 1.15, -3.4]} size={[2.2, 0.7]} lines={["WEB", `${CHALLENGES[1]?.points ?? 0}`]} />
        <Screen position={[4.4, 1.15, -3.4]} size={[2.2, 0.7]} lines={["CRYPTO", `${CHALLENGES[2]?.points ?? 0}`]} />
        <Screen position={[0, 3.15, -5.84]} size={[6.4, 1.5]} lines={["SCORE", "CONTRACT  100", "WEB  150", "CRYPTO  80"]} />
      </>
    );
  }
  if (id === "skin-forge") {
    return (
      <>
        <Screen position={[-3.5, 2.35, -5.84]} size={[4.6, 2.3]} lines={["HALL", `NEXT  ${countdown}`, "VAULT  HEIST", "FLOOR  PERPS", "WAR  SANDBOX"]} />
        <Screen position={[3.6, 2.25, -5.84]} size={[4.2, 2.1]} lines={rankLines} />
      </>
    );
  }
  return <Screen position={[0, 2.55, -5.78]} size={[6.6, 2.3]} lines={["CUP", "MASON  2", "SCOUT  1", "TAPE  WATCH"]} />;
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

export function Interiors({ focusId, frameMode }: { focusId: string | null; frameMode: "in" | "out" }) {
  if (frameMode !== "in" || !focusId || !ROOMS[focusId]) return null;
  const room = ROOMS[focusId];
  return (
    <Suspense fallback={null}>
      <group position={room.position}>
        <RoomMesh key={room.url} url={room.url} />
        {LAMPS.map(([x, y, z], index) => (
          <pointLight key={index} position={[x, y, z]} color="#e7b07a" intensity={22} distance={9} decay={2} />
        ))}
        <RoomBoards id={focusId} />
        {focusId === "cup-stadium" ? <SeatCrowd /> : null}
      </group>
    </Suspense>
  );
}

for (const room of Object.values(ROOMS)) useGLTF.preload(room.url);
