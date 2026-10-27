"use client";

import { useGLTF } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";
import { Suspense, useLayoutEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import { CHALLENGES, formatCountdown, heistCountdown, heistRows, mockQuotes } from "@/lib/schedule";

const ROOMS: Record<string, { url: string; position: [number, number, number] }> = {
  "vault-quarter": { url: "/models/interior-vault.glb", position: [-42, 0, 18] },
  "trading-village": { url: "/models/interior-market.glb", position: [46, 0, 14] },
  "war-room": { url: "/models/interior-war.glb", position: [12, 0, 40] },
  "skin-forge": { url: "/models/interior-forge.glb", position: [-38, 0, -24] },
  "cup-stadium": { url: "/models/interior-stadium.glb", position: [-8, 0, -42] },
};

const ROOM_LIGHTS: Record<string, Array<{ at: [number, number, number]; color: string; intensity: number; distance: number }>> = {
  "vault-quarter": [
    { at: [-3.4, 2.85, -1.6], color: "#d7dbe2", intensity: 14, distance: 6.5 },
    { at: [3.4, 2.85, -1.6], color: "#d7dbe2", intensity: 14, distance: 6.5 },
    { at: [0, 2.8, 1.2], color: "#c5cad2", intensity: 9, distance: 5.5 },
  ],
  "trading-village": [
    { at: [-3.2, 5.4, -1.2], color: "#d5dde6", intensity: 20, distance: 12 },
    { at: [3.2, 5.4, -1.2], color: "#d5dde6", intensity: 20, distance: 12 },
    { at: [0, 5.1, 2.2], color: "#c9d3de", intensity: 14, distance: 10 },
  ],
  "war-room": [
    { at: [0, 1.5, -0.4], color: "#e7b07a", intensity: 7, distance: 4.5 },
    { at: [0, 2.6, -3.6], color: "#d4d2cc", intensity: 5, distance: 6 },
  ],
  "skin-forge": [
    { at: [0, 6.2, -0.6], color: "#e7c9a4", intensity: 26, distance: 16 },
    { at: [-3.2, 4.4, -1.5], color: "#e7b07a", intensity: 12, distance: 9 },
    { at: [3.2, 4.4, -1.5], color: "#e7b07a", intensity: 12, distance: 9 },
  ],
  "cup-stadium": [
    { at: [-1.7, 5.3, -2.5], color: "#f0d2a8", intensity: 28, distance: 11 },
    { at: [1.7, 5.3, -2.5], color: "#f0d2a8", intensity: 28, distance: 11 },
    { at: [0, 5.1, 0.5], color: "#e7b07a", intensity: 16, distance: 9 },
  ],
};

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
        else if (name.includes("beam")) {
          standard.transparent = true;
          standard.opacity = 0.16;
          standard.depthWrite = false;
          standard.emissive = new THREE.Color("#e7b07a");
          standard.emissiveIntensity = 0.7;
        } else if (name.includes("glow")) standard.emissiveIntensity = Math.min(standard.emissiveIntensity, 0.32);
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

type BoardRow = { label: string; value?: string; tone?: "green" | "orange" };

function toneColor(tone: BoardRow["tone"]) {
  if (tone === "green") return "#2fb86a";
  if (tone === "orange") return "#c94a1a";
  return "#d4d2cc";
}

function paintBoard(canvas: HTMLCanvasElement, title: string, rows: BoardRow[], compact: boolean, band: boolean) {
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const display = quotedFamily("--font-display", "sans-serif");
  const mono = quotedFamily("--font-mono", "monospace");
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = "#141416";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  if (band) {
    ctx.textBaseline = "middle";
    ctx.fillStyle = "#d4d2cc";
    ctx.font = `700 72px "${display}", sans-serif`;
    ctx.fillText(title, 28, canvas.height / 2);
    let x = 28 + ctx.measureText(title).width + 36;
    ctx.font = `500 56px "${mono}", monospace`;
    rows.forEach((row) => {
      ctx.fillStyle = "#8b8a86";
      ctx.fillText(row.label, x, canvas.height / 2);
      x += ctx.measureText(row.label).width + 14;
      if (!row.value) return;
      ctx.fillStyle = toneColor(row.tone);
      ctx.fillText(row.value, x, canvas.height / 2);
      x += ctx.measureText(row.value).width + 32;
    });
    return;
  }
  const titleSize = compact ? 150 : 84;
  const bodySize = compact ? 132 : 60;
  const titleY = compact ? 48 : 40;
  const bodyTop = compact ? 280 : 168;
  const bodyGap = compact ? 180 : 78;
  ctx.textBaseline = "top";
  ctx.fillStyle = "#d4d2cc";
  ctx.font = `700 ${titleSize}px "${display}", sans-serif`;
  if (title) ctx.fillText(title, 48, titleY);
  ctx.font = `500 ${bodySize}px "${mono}", monospace`;
  rows.slice(0, 5).forEach((row, index) => {
    const y = bodyTop + index * bodyGap;
    ctx.fillStyle = "#8b8a86";
    ctx.fillText(row.label, 48, y);
    if (!row.value) return;
    ctx.fillStyle = toneColor(row.tone);
    const labelWidth = ctx.measureText(`${row.label} `).width;
    ctx.fillText(row.value, 48 + labelWidth + 18, y);
  });
}

function Screen({
  position,
  title,
  rows = [],
  size = [3.4, 1.92],
  rotation = [0, 0, 0],
}: {
  position: [number, number, number];
  title: string;
  rows?: BoardRow[];
  size?: [number, number];
  rotation?: [number, number, number];
}) {
  const key = `${title}|${rows.map((row) => `${row.label}:${row.value ?? ""}:${row.tone ?? ""}`).join("|")}`;
  const [width, height] = size;
  const compact = height < 0.85;
  const band = width / height > 8;
  const map = useMemo(() => {
    const canvas = document.createElement("canvas");
    canvas.width = band ? 3072 : 1024;
    canvas.height = band ? 384 : 576;
    const texture = new THREE.CanvasTexture(canvas);
    texture.colorSpace = THREE.SRGBColorSpace;
    texture.flipY = true;
    texture.anisotropy = 8;
    texture.needsUpdate = true;
    return texture;
  }, [band]);
  useLayoutEffect(() => {
    let dead = false;
    const draw = () => {
      paintBoard(map.image as HTMLCanvasElement, title, rows, compact, band);
      map.needsUpdate = true;
    };
    draw();
    const display = quotedFamily("--font-display", "sans-serif");
    const mono = quotedFamily("--font-mono", "monospace");
    const fonts = Promise.all([
      document.fonts.load(`700 ${band ? 72 : compact ? 150 : 84}px "${display}"`),
      document.fonts.load(`500 ${band ? 56 : compact ? 132 : 60}px "${mono}"`),
    ]).catch(() => undefined);
    const raced = Promise.race([fonts, new Promise((resolve) => window.setTimeout(resolve, 400))]);
    raced.then(() => {
      if (!dead) draw();
    });
    return () => {
      dead = true;
    };
    // key is the line text. Repaint when it changes, including after the font load.
  }, [key, map, compact, band, title]);
  return (
    <group position={position} rotation={rotation}>
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
    if (next === last.current) return;
    last.current = next;
    setNow(next);
  });
  return now;
}

function RoomBoards({ id }: { id: string }) {
  const now = useClock();
  const clock = heistCountdown(now);
  const clockRow: BoardRow = {
    label: clock.live ? "LIVE" : "NEXT",
    value: formatCountdown(clock.secs),
    tone: clock.live ? "green" : "orange",
  };
  const ranks = heistRows("season", now).slice(0, 4);
  const rankRows: BoardRow[] = ranks.map((row, index) => ({
    label: `${index + 1}  ${row.name.toUpperCase()}`,
    value: String(row.points),
    tone: "orange",
  }));
  const quotes = mockQuotes(now);
  if (id === "vault-quarter") {
    return (
      <>
        <Screen
          position={[-3.6, 1.85, -5.9]}
          size={[2.8, 1.35]}
          title="SCHEDULE"
          rows={[clockRow, { label: "DAILY", value: "18:00Z" }, { label: "WEEK", value: "MON" }, { label: "SEASON", value: "1" }]}
        />
        <Screen position={[3.6, 1.85, -5.9]} size={[2.8, 1.35]} title="SEASON" rows={rankRows} />
      </>
    );
  }
  if (id === "trading-village") {
    return (
      <>
        <Screen
          position={[0, 4.55, -5.78]}
          size={[11.4, 0.7]}
          title="TICKER"
          rows={quotes.map((row) => ({
            label: row.symbol,
            value: (row.mark / 1_000_000).toFixed(2),
            tone: row.pnlMicros >= 0 ? "green" : "orange",
          }))}
        />
        <Screen
          position={[5.55, 2.35, -5.78]}
          size={[2.2, 1.5]}
          title="PNL"
          rows={quotes.map((row) => ({
            label: `${row.fundingBps} BPS`,
            value: (row.pnlMicros / 1_000_000).toFixed(2),
            tone: row.pnlMicros >= 0 ? "green" : "orange",
          }))}
        />
      </>
    );
  }
  if (id === "war-room") {
    return (
      <>
        <Screen position={[-4.4, 2.15, -3.4]} size={[2.2, 0.7]} title="CONTRACT" rows={[{ label: "", value: `${CHALLENGES[0]?.points ?? 0}`, tone: "orange" }]} />
        <Screen position={[0, 2.15, -3.4]} size={[2.2, 0.7]} title="WEB" rows={[{ label: "", value: `${CHALLENGES[1]?.points ?? 0}`, tone: "orange" }]} />
        <Screen position={[4.4, 2.15, -3.4]} size={[2.2, 0.7]} title="CRYPTO" rows={[{ label: "", value: `${CHALLENGES[2]?.points ?? 0}`, tone: "orange" }]} />
        <Screen
          position={[0, 2.95, -5.82]}
          size={[6.2, 1.05]}
          title="SCORE"
          rows={[
            { label: "CONTRACT", value: "100", tone: "orange" },
            { label: "WEB", value: "150", tone: "green" },
            { label: "CRYPTO", value: "80", tone: "orange" },
          ]}
        />
      </>
    );
  }
  if (id === "skin-forge") {
    return (
      <>
        <Screen
          position={[-2.6, 5.15, -5.78]}
          size={[4.6, 1.35]}
          title="HALL"
          rows={[clockRow, { label: "VAULT", value: "HEIST" }, { label: "FLOOR", value: "PERPS" }, { label: "WAR", value: "SANDBOX" }]}
        />
        <Screen position={[2.7, 5.15, -5.78]} size={[4.4, 1.35]} title="SEASON" rows={rankRows} />
        <Screen position={[-4.7, 2.55, -5.72]} size={[1.25, 0.42]} title="VAULT" />
        <Screen position={[-1.6, 2.55, -5.72]} size={[1.25, 0.42]} title="CUP" />
        <Screen position={[1.6, 2.55, -5.72]} size={[1.25, 0.42]} title="FLOOR" />
        <Screen position={[4.7, 2.55, -5.72]} size={[1.25, 0.42]} title="WAR" />
      </>
    );
  }
  return (
    <Screen
      position={[0, 4.15, -1.15]}
      size={[4.2, 1.2]}
      title="CUP"
      rows={[
        { label: "MASON", value: "2", tone: "green" },
        { label: "SCOUT", value: "1", tone: "orange" },
        { label: "TAPE", value: "WATCH" },
      ]}
    />
  );
}

function SeatCrowd() {
  const bodies = useRef<THREE.InstancedMesh>(null);
  const heads = useRef<THREE.InstancedMesh>(null);
  const spots = useMemo(() => {
    const list: Array<[number, number, number]> = [];
    const north: Array<[number, number]> = [[-3.55, 0.32], [-4.4, 0.72], [-5.2, 1.12]];
    north.forEach(([z, y]) => {
      for (let seat = 0; seat < 7; seat += 1) list.push([-2.55 + seat * 0.85, y + 0.58, z]);
    });
    const sides: Array<[number, number]> = [[4.55, 0.38], [5.5, 0.8]];
    sides.forEach(([x, y]) => {
      for (const z of [-2.2, -1.3, -0.4, 0.5, 1.4]) {
        list.push([x, y + 0.58, z]);
        list.push([-x, y + 0.58, z]);
      }
    });
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
        {(ROOM_LIGHTS[focusId] ?? []).map((lamp) => (
          <pointLight key={lamp.at.join(",")} position={lamp.at} color={lamp.color} intensity={lamp.intensity} distance={lamp.distance} decay={2} />
        ))}
        <RoomBoards id={focusId} />
        {focusId === "cup-stadium" ? <SeatCrowd /> : null}
      </group>
    </Suspense>
  );
}

for (const room of Object.values(ROOMS)) useGLTF.preload(room.url);
