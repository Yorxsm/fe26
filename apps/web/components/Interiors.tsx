"use client";

import { useGLTF } from "@react-three/drei";
import { Suspense, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import { FRAME_BOARDS, type BoardSign, type FrameBoard } from "@/lib/boards";
import { useHeistNow } from "@/lib/clock";
import { apiBase } from "@/lib/protocol";
import { CHALLENGES, formatCountdown, heistCountdown, mockQuotes } from "@/lib/schedule";

const ROOMS: Record<string, { url: string; position: [number, number, number] }> = {
  "vault-quarter": { url: "/models/interior-vault.glb", position: [-42, 0, 18] },
  "trading-village": { url: "/models/interior-market.glb", position: [46, 0, 14] },
  "war-room": { url: "/models/interior-war.glb", position: [12, 0, 40] },
  "skin-forge": { url: "/models/interior-forge.glb", position: [-38, 0, -24] },
  "cup-stadium": { url: "/models/interior-stadium.glb", position: [-8, 0, -42] },
};

const ROOM_LIGHTS: Record<string, Array<{ at: [number, number, number]; color: string; intensity: number; distance: number }>> = {
  "vault-quarter": [
    { at: [-3.4, 2.85, -1.6], color: "#d7dbe2", intensity: 11.5, distance: 6.5 },
    { at: [3.4, 2.85, -1.6], color: "#d7dbe2", intensity: 11.5, distance: 6.5 },
    { at: [0, 2.8, 1.2], color: "#c5cad2", intensity: 7.4, distance: 5.5 },
  ],
  "trading-village": [
    { at: [-3.2, 5.4, -1.2], color: "#d5dde6", intensity: 16.4, distance: 12 },
    { at: [3.2, 5.4, -1.2], color: "#d5dde6", intensity: 16.4, distance: 12 },
    { at: [0, 5.1, 2.2], color: "#c9d3de", intensity: 11.5, distance: 10 },
  ],
  "war-room": [
    { at: [0, 1.35, -0.4], color: "#1f6b45", intensity: 2.2, distance: 3.4 },
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
        if (name.includes("beam")) mesh.visible = false;
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

type BoardRow = { label: string; value?: string; tone?: "green" | "orange" };

function toneColor(tone: BoardRow["tone"]) {
  if (tone === "green") return "#2fb86a";
  if (tone === "orange") return "#c94a1a";
  return "#d4d2cc";
}

const SIGN_COPY: Record<BoardSign, { kicker: string; name: string }> = {
  vault: { kicker: "VAULT", name: "HEIST" },
  cup: { kicker: "TRADING", name: "CUP" },
  floor: { kicker: "FLOOR", name: "PERPS" },
  war: { kicker: "WAR", name: "SANDBOX" },
};

function drawSignIcon(ctx: CanvasRenderingContext2D, sign: BoardSign, cx: number, cy: number) {
  ctx.strokeStyle = "#d4d2cc";
  ctx.fillStyle = "#d4d2cc";
  ctx.lineWidth = 16;
  ctx.lineJoin = "round";
  ctx.lineCap = "round";
  if (sign === "vault") {
    ctx.strokeRect(cx - 70, cy - 96, 140, 192);
    ctx.beginPath();
    ctx.arc(cx + 36, cy, 14, 0, Math.PI * 2);
    ctx.stroke();
    return;
  }
  if (sign === "cup") {
    ctx.beginPath();
    ctx.moveTo(cx - 78, cy - 78);
    ctx.lineTo(cx - 48, cy + 36);
    ctx.lineTo(cx + 48, cy + 36);
    ctx.lineTo(cx + 78, cy - 78);
    ctx.moveTo(cx - 36, cy + 36);
    ctx.lineTo(cx - 22, cy + 86);
    ctx.lineTo(cx + 22, cy + 86);
    ctx.lineTo(cx + 36, cy + 36);
    ctx.stroke();
    return;
  }
  if (sign === "floor") {
    [78, 140, 104].forEach((bar, index) => {
      ctx.fillRect(cx - 86 + index * 62, cy + 86 - bar, 42, bar);
    });
    return;
  }
  ctx.beginPath();
  ctx.moveTo(cx, cy - 96);
  ctx.lineTo(cx + 78, cy);
  ctx.lineTo(cx, cy + 96);
  ctx.lineTo(cx - 78, cy);
  ctx.closePath();
  ctx.stroke();
}

function paintSign(canvas: HTMLCanvasElement, sign: BoardSign, status: string, tone: BoardRow["tone"]) {
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const display = quotedFamily("--font-display", "sans-serif");
  const mono = quotedFamily("--font-mono", "monospace");
  const copy = SIGN_COPY[sign];
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = "#141416";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = "#1e1e21";
  ctx.fillRect(28, 28, canvas.width - 56, canvas.height - 56);
  drawSignIcon(ctx, sign, 168, canvas.height / 2);
  ctx.textBaseline = "top";
  ctx.fillStyle = "#8b8a86";
  ctx.font = `500 64px "${mono}", monospace`;
  ctx.fillText(copy.kicker, 320, 78);
  ctx.fillStyle = "#d4d2cc";
  ctx.font = `700 168px "${display}", sans-serif`;
  ctx.fillText(copy.name, 320, 168);
  ctx.fillStyle = toneColor(tone);
  let statusSize = 112;
  ctx.font = `500 ${statusSize}px "${mono}", monospace`;
  while (statusSize > 72 && ctx.measureText(status).width > canvas.width - 380) {
    statusSize -= 4;
    ctx.font = `500 ${statusSize}px "${mono}", monospace`;
  }
  ctx.fillText(status, 320, 400);
}

function paintBoard(canvas: HTMLCanvasElement, title: string, rows: BoardRow[], compact: boolean, band: boolean, inset: boolean) {
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
  const titleSize = compact ? 150 : inset ? 64 : 84;
  const bodySize = compact ? 132 : inset ? 48 : 60;
  const topPad = Math.round(canvas.height * (inset ? 0.2 : 0.06));
  const titleY = inset ? topPad : compact ? 48 : 40;
  const bodyTop = inset ? topPad + titleSize + 28 : compact ? 280 : 168;
  const bodyGap = compact ? 180 : inset ? 64 : 78;
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
  sign,
  status = "",
  tone = "green",
}: {
  position: [number, number, number];
  title: string;
  rows?: BoardRow[];
  size?: [number, number];
  rotation?: [number, number, number];
  sign?: BoardSign;
  status?: string;
  tone?: BoardRow["tone"];
}) {
  const key = sign ? `${sign}|${status}|${tone}` : `${title}|${rows.map((row) => `${row.label}:${row.value ?? ""}:${row.tone ?? ""}`).join("|")}`;
  const [width, height] = size;
  const compact = !sign && height < 0.85;
  const band = !sign && width / height > 8;
  const inset = !sign && height < 1.4;
  const map = useMemo(() => {
    const canvas = document.createElement("canvas");
    canvas.width = sign ? 1024 : band ? 3072 : 1024;
    canvas.height = sign ? 640 : band ? 384 : 576;
    const texture = new THREE.CanvasTexture(canvas);
    texture.colorSpace = THREE.SRGBColorSpace;
    texture.flipY = true;
    texture.anisotropy = 8;
    texture.needsUpdate = true;
    return texture;
  }, [band, sign]);
  useLayoutEffect(() => {
    let dead = false;
    const draw = () => {
      const canvas = map.image as HTMLCanvasElement;
      if (sign) paintSign(canvas, sign, status, tone);
      else paintBoard(canvas, title, rows, compact, band, inset);
      map.needsUpdate = true;
    };
    draw();
    const display = quotedFamily("--font-display", "sans-serif");
    const mono = quotedFamily("--font-mono", "monospace");
    const fonts = Promise.all([
      document.fonts.load(`700 ${sign ? 168 : band ? 72 : compact ? 150 : inset ? 64 : 84}px "${display}"`),
      document.fonts.load(`500 ${sign ? 112 : band ? 56 : compact ? 132 : inset ? 48 : 60}px "${mono}"`),
    ]).catch(() => undefined);
    const raced = Promise.race([fonts, new Promise((resolve) => window.setTimeout(resolve, 400))]);
    raced.then(() => {
      if (!dead) draw();
    });
    return () => {
      dead = true;
    };
    // key is the line text. Repaint when it changes, including after the font load.
  }, [key, map, compact, band, title, sign, status, tone]);
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
  return useHeistNow();
}

function placed(id: string, name: string): FrameBoard {
  const board = FRAME_BOARDS[id]?.find((entry) => entry.name === name);
  if (!board) throw new Error(`${id} ${name}`);
  return board;
}

const BOARD_ROOM: Record<string, string> = {
  "vault-quarter": "vault",
  "cup-stadium": "cup",
  "trading-village": "village",
  "war-room": "war",
  "skin-forge": "all",
};

function useSeasonBoard(id: string) {
  const room = BOARD_ROOM[id] ?? "all";
  const [rows, setRows] = useState<BoardRow[] | null>(null);
  useEffect(() => {
    const base = apiBase();
    if (!base) return;
    let dead = false;
    fetch(`${base}/v1/leaderboard?window=season&room=${room}`, { cache: "no-store" })
      .then((response) => (response.ok ? response.json() : Promise.reject(new Error("missing"))))
      .then((body: { rows?: Array<{ name?: string; points?: number }> }) => {
        if (dead) return;
        const next = (body.rows ?? []).slice(0, 4).map((row, index) => ({
          label: `${index + 1}  ${String(row.name ?? "").toUpperCase()}`,
          value: String(row.points ?? ""),
          tone: "orange" as const,
        }));
        setRows(next.length ? next : [{ label: "LIVE DATA UNAVAILABLE" }]);
      })
      .catch(() => {
        if (!dead) setRows([{ label: "LIVE DATA UNAVAILABLE" }]);
      });
    return () => {
      dead = true;
    };
  }, [room]);
  return rows ?? [{ label: "…" }];
}

function RoomBoards({ id }: { id: string }) {
  const now = useClock();
  const clock = now == null ? null : heistCountdown(now);
  const clockRow: BoardRow = {
    label: clock?.live ? "LIVE" : "NEXT",
    value: clock ? formatCountdown(clock.secs) : "…",
    tone: clock?.live ? "green" : "orange",
  };
  const rankRows = useSeasonBoard(id);
  const quotes = mockQuotes(now ?? 0);
  const screen = (name: string, rows: BoardRow[]) => {
    const board = placed(id, name);
    return <Screen key={name} position={board.at} size={board.size} title={name} rows={rows} />;
  };
  if (id === "vault-quarter") {
    return (
      <>
        {screen("SCHEDULE", [clockRow, { label: "DAILY", value: "18:00Z" }, { label: "WEEK", value: "MON" }, { label: "SEASON", value: "1" }])}
        {screen("SEASON", rankRows)}
      </>
    );
  }
  if (id === "trading-village") {
    return (
      <>
        {screen(
          "TICKER",
          quotes.map((row) => ({
            label: row.symbol,
            value: (row.mark / 1_000_000).toFixed(2),
            tone: row.pnlMicros >= 0 ? "green" : "orange",
          })),
        )}
        {screen(
          "PNL",
          quotes.map((row) => ({
            label: `${row.fundingBps} BPS`,
            value: (row.pnlMicros / 1_000_000).toFixed(2),
            tone: row.pnlMicros >= 0 ? "green" : "orange",
          })),
        )}
      </>
    );
  }
  if (id === "war-room") {
    return (
      <>
        {screen("CONTRACT", [{ label: "", value: `${CHALLENGES[0]?.points ?? 0}`, tone: "orange" }])}
        {screen("WEB", [{ label: "", value: `${CHALLENGES[1]?.points ?? 0}`, tone: "orange" }])}
        {screen("CRYPTO", [{ label: "", value: `${CHALLENGES[2]?.points ?? 0}`, tone: "orange" }])}
        {screen("SCORE", [
          { label: "CONTRACT", value: "100", tone: "orange" },
          { label: "WEB", value: "150", tone: "green" },
          { label: "CRYPTO", value: "80", tone: "orange" },
        ])}
        <HoloMap />
      </>
    );
  }
  if (id === "skin-forge") {
    const vaultStatus = clock ? `${clock.live ? "LIVE" : "NEXT"} ${formatCountdown(clock.secs)}` : "NEXT";
    return (
      <>
        {screen("HALL", [clockRow, { label: "VAULT", value: "HEIST" }, { label: "FLOOR", value: "PERPS" }, { label: "WAR", value: "SANDBOX" }])}
        {screen("SEASON", rankRows)}
        {FRAME_BOARDS["skin-forge"].filter((board) => board.sign).map((board) => (
          <Screen
            key={board.name}
            position={board.at}
            size={board.size}
            title={board.name}
            sign={board.sign}
            status={board.sign === "vault" ? vaultStatus : "LIVE"}
            tone={board.sign === "vault" && !clock?.live ? "orange" : "green"}
          />
        ))}
      </>
    );
  }
  const cup = placed(id, "CUP");
  return (
    <>
      <Screen
        position={cup.at}
        size={cup.size}
        title="CUP"
        rows={rankRows}
      />
      <StadiumBeams />
    </>
  );
}

function paintHolo(canvas: HTMLCanvasElement) {
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = "rgba(16, 64, 42, 0.72)";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.strokeStyle = "rgba(47, 184, 106, 0.45)";
  ctx.lineWidth = 2;
  for (let y = 4; y < canvas.height; y += 10) {
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(canvas.width, y);
    ctx.stroke();
  }
  ctx.strokeStyle = "rgba(47, 184, 106, 0.85)";
  ctx.lineWidth = 3;
  for (let i = 36; i < canvas.width; i += 68) {
    ctx.beginPath();
    ctx.moveTo(i, 28);
    ctx.lineTo(i, canvas.height - 28);
    ctx.moveTo(28, i);
    ctx.lineTo(canvas.width - 28, i);
    ctx.stroke();
  }
  ctx.strokeStyle = "#2fb86a";
  ctx.lineWidth = 6;
  ctx.beginPath();
  ctx.moveTo(48, 150);
  ctx.bezierCurveTo(140, 90, 210, 250, 320, 180);
  ctx.bezierCurveTo(410, 120, 460, 220, 500, 170);
  ctx.moveTo(180, 70);
  ctx.bezierCurveTo(160, 180, 240, 300, 200, 420);
  ctx.stroke();
}

function HoloMap() {
  const material = useMemo(() => {
    const canvas = document.createElement("canvas");
    canvas.width = 512;
    canvas.height = 512;
    paintHolo(canvas);
    const map = new THREE.CanvasTexture(canvas);
    map.colorSpace = THREE.SRGBColorSpace;
    map.needsUpdate = true;
    return new THREE.MeshBasicMaterial({
      map,
      color: "#ffffff",
      transparent: true,
      depthWrite: false,
      side: THREE.DoubleSide,
      toneMapped: false,
    });
  }, []);
  return (
    <mesh position={[0, 1.12, -0.4]} rotation={[-Math.PI / 2, 0, 0]} renderOrder={4}>
      <planeGeometry args={[1.5, 1.5]} />
      <primitive object={material} attach="material" />
    </mesh>
  );
}

function StadiumBeams() {
  const material = useMemo(() => {
    const mat = new THREE.ShaderMaterial({
      transparent: true,
      depthWrite: false,
      depthTest: true,
      side: THREE.DoubleSide,
      blending: THREE.AdditiveBlending,
      toneMapped: false,
      uniforms: { uHalf: { value: 2.2 }, uGain: { value: 1 } },
      vertexShader: `
        varying float vY;
        varying vec3 vNormal;
        void main() {
          vY = position.y;
          vNormal = normalize(normalMatrix * normal);
          gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
        }
      `,
      fragmentShader: `
        varying float vY;
        varying vec3 vNormal;
        uniform float uHalf;
        uniform float uGain;
        void main() {
          float t = clamp(vY / uHalf * 0.5 + 0.5, 0.0, 1.0);
          float along = smoothstep(0.0, 0.32, t) * smoothstep(1.0, 0.52, t);
          float rim = pow(1.0 - abs(normalize(vNormal).z), 1.4);
          float alpha = along * (0.04 + 0.18 * rim) * uGain;
          gl_FragColor = vec4(0.91, 0.79, 0.64, alpha);
        }
      `,
    });
    return mat;
  }, []);
  const spots: Array<[number, number, number]> = [
    [-2.35, 3.35, -0.2],
    [2.4, 3.35, -2.9],
    [0.1, 3.35, 0.7],
  ];
  return (
    <>
      {spots.map((at) => (
        <mesh key={at.join(",")} position={at} material={material} renderOrder={2}>
          <coneGeometry args={[0.58, 4.4, 18, 1, true]} />
        </mesh>
      ))}
    </>
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
