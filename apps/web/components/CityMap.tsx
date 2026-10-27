"use client";

import { Html, OrbitControls, PerformanceMonitor } from "@react-three/drei";
import { Canvas, useFrame, useThree } from "@react-three/fiber";
import { useLayoutEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import { CITY_AGENTS, DISTRICTS, placeAgent, type District } from "@/lib/city";

const BEATS: Array<{ position: [number, number, number]; target: [number, number, number]; zoom: number }> = [
  { position: [0, 20, 16], target: [0, 0, 0], zoom: 22 },
  { position: [-8, 10, -1], target: [-10.5, 0.7, -4], zoom: 58 },
  { position: [4, 14, 10], target: [3.5, 0.3, 4], zoom: 36 },
  { position: [0, 11, 2], target: [0, 0.4, -4], zoom: 44 },
];

type Rig = { target: THREE.Vector3; update: () => void; autoRotate: boolean; enabled: boolean };

const dummy = new THREE.Object3D();
const tint = new THREE.Color();
const aim = new THREE.Vector3();
const eye = new THREE.Vector3();

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

function writeInstance(
  mesh: THREE.InstancedMesh,
  index: number,
  x: number,
  y: number,
  z: number,
  rotY: number,
  sx: number,
  sy: number,
  sz: number,
  color?: string,
) {
  dummy.position.set(x, y, z);
  dummy.rotation.set(0, rotY, 0);
  dummy.scale.set(sx, sy, sz);
  dummy.updateMatrix();
  mesh.setMatrixAt(index, dummy.matrix);
  if (color) mesh.setColorAt(index, tint.set(color));
}

function ApplyToon({ map }: { map: THREE.Texture | null }) {
  const scene = useThree((state) => state.scene);
  useLayoutEffect(() => {
    if (!map) return;
    scene.traverse((node) => {
      const mesh = node as THREE.Mesh;
      if (!mesh.material) return;
      const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
      materials.forEach((material) => {
        const toon = material as THREE.MeshToonMaterial;
        if (toon.isMeshToonMaterial) {
          toon.gradientMap = map;
          toon.needsUpdate = true;
        }
      });
    });
  }, [map, scene]);
  return null;
}

function Rooms({ onPick }: { onPick: (district: District) => void }) {
  const floors = useRef<THREE.InstancedMesh>(null);
  const shells = useRef<THREE.InstancedMesh>(null);
  const paths = useRef<THREE.InstancedMesh>(null);
  const corridors = useMemo(() => {
    const links: Array<{ x: number; z: number; rot: number; len: number }> = [];
    DISTRICTS.forEach((room, index) => {
      const east = DISTRICTS[index + 1];
      if (east && Math.floor(index / 4) === Math.floor((index + 1) / 4)) {
        links.push({
          x: (room.x + east.x) / 2,
          z: room.z,
          rot: Math.PI / 2,
          len: Math.abs(east.x - room.x) - 2.4,
        });
      }
      const south = DISTRICTS[index + 4];
      if (south) {
        links.push({
          x: room.x,
          z: (room.z + south.z) / 2,
          rot: 0,
          len: Math.abs(south.z - room.z) - 2.4,
        });
      }
    });
    return links;
  }, []);

  useLayoutEffect(() => {
    const floor = floors.current;
    const shell = shells.current;
    const path = paths.current;
    if (!floor || !shell || !path) return;
    DISTRICTS.forEach((room, index) => {
      writeInstance(floor, index, room.x, 0.16, room.z, 0, 1, 1, 1, room.color);
      writeInstance(shell, index, room.x, 0.16, room.z, 0, 1.08, 1.2, 1.08);
    });
    corridors.forEach((link, index) => {
      writeInstance(path, index, link.x, 0.08, link.z, link.rot, 1.2, 1, link.len);
    });
    floor.instanceMatrix.needsUpdate = true;
    shell.instanceMatrix.needsUpdate = true;
    path.instanceMatrix.needsUpdate = true;
    if (floor.instanceColor) floor.instanceColor.needsUpdate = true;
  }, [corridors]);

  return (
    <group>
      <instancedMesh ref={shells} args={[undefined, undefined, DISTRICTS.length]} frustumCulled={false}>
        <boxGeometry args={[4.7, 0.36, 4.7]} />
        <meshBasicMaterial color="#14181f" side={THREE.BackSide} />
      </instancedMesh>
      <instancedMesh
        ref={floors}
        args={[undefined, undefined, DISTRICTS.length]}
        frustumCulled={false}
        onClick={(event) => {
          event.stopPropagation();
          const id = event.instanceId;
          if (id === undefined || id === null) return;
          onPick(DISTRICTS[id]);
        }}
      >
        <boxGeometry args={[4.4, 0.28, 4.4]} />
        <meshToonMaterial color="#ffffff" />
      </instancedMesh>
      <instancedMesh ref={paths} args={[undefined, undefined, Math.max(corridors.length, 1)]} frustumCulled={false}>
        <boxGeometry args={[1, 0.14, 1]} />
        <meshToonMaterial color="#4a5563" />
      </instancedMesh>
      {DISTRICTS.map((room) => (
        <group key={room.id} position={[room.x, 0.46, room.z]}>
          <mesh
            position={[0, 0.12, 0]}
            onClick={(event) => {
              event.stopPropagation();
              onPick(room);
            }}
          >
            <cylinderGeometry args={[0.26, 0.32, 0.24, 8]} />
            <meshToonMaterial color="#f4e1c1" />
          </mesh>
          <mesh position={[0, 0.3, 0]}>
            <boxGeometry args={[0.24, 0.08, 0.16]} />
            <meshBasicMaterial color={room.status === "live" ? "#f2c14e" : "#5c6570"} />
          </mesh>
          <Html center position={[0, 0.9, 0]} zIndexRange={[2, 0]} style={{ pointerEvents: "none" }}>
            <div className="station-tag">{room.station}</div>
          </Html>
        </group>
      ))}
    </group>
  );
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
  useLayoutEffect(() => {
    reduce.current = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }, []);

  useFrame((state, delta) => {
    const rig = controls;
    if (!rig?.target || !(camera instanceof THREE.OrthographicCamera)) return;
    const driving =
      mode === "cutscene" || (mode === "play" && (followIndex !== null || Boolean(focusId)));
    rig.enabled = !driving;
    rig.autoRotate = mode === "orbit" && !reduce.current;
    if (mode === "orbit") {
      aim.set(0, 0, 0);
      rig.target.lerp(aim, reduce.current ? 1 : 0.08);
      camera.zoom = THREE.MathUtils.damp(camera.zoom, 24, reduce.current ? 40 : 2, delta);
      camera.updateProjectionMatrix();
      return;
    }
    let zoom = 26;
    let drive = false;
    if (mode === "cutscene") {
      const shot = BEATS[Math.min(beat, BEATS.length - 1)];
      aim.set(shot.target[0], shot.target[1], shot.target[2]);
      eye.set(shot.position[0], shot.position[1], shot.position[2]);
      zoom = shot.zoom;
      drive = true;
    } else if (followIndex !== null) {
      const pose = placeAgent(followIndex, state.clock.elapsedTime, paused);
      aim.set(pose.x, 0, pose.z);
      eye.set(pose.x, 16, pose.z + 10);
      zoom = 72;
      drive = true;
    } else if (focusId) {
      const room = DISTRICTS.find((district) => district.id === focusId);
      if (room) {
        aim.set(room.x, 0, room.z);
        eye.set(room.x, 16, room.z + 10);
        zoom = 58;
        drive = true;
      }
    }
    if (!drive) return;
    const blend = reduce.current ? 1 : 1 - Math.exp(-delta * 3.2);
    rig.target.lerp(aim, blend);
    camera.position.lerp(eye, blend);
    camera.lookAt(rig.target);
    camera.zoom = THREE.MathUtils.damp(camera.zoom, zoom, reduce.current ? 40 : 3, delta);
    camera.updateProjectionMatrix();
  });
  return null;
}

function Agents({
  paused,
  audit,
  onFollow,
}: {
  paused: boolean;
  audit: boolean;
  onFollow: (index: number) => void;
}) {
  const bodies = useRef<THREE.InstancedMesh>(null);
  const shells = useRef<THREE.InstancedMesh>(null);
  const legs = useRef<THREE.InstancedMesh>(null);
  const cores = useRef<THREE.InstancedMesh>(null);
  const bolts = useRef<THREE.InstancedMesh>(null);
  const rings = useRef<THREE.InstancedMesh>(null);
  const tags = useRef<Array<THREE.Group | null>>([]);
  const bodyGeo = useMemo(() => {
    const geometry = new THREE.CylinderGeometry(0.34, 0.34, 1.05, 8);
    geometry.rotateZ(Math.PI / 2);
    return geometry;
  }, []);
  const shellGeo = useMemo(() => {
    const geometry = new THREE.CylinderGeometry(0.34, 0.34, 1.05, 8);
    geometry.rotateZ(Math.PI / 2);
    return geometry;
  }, []);
  const ringGeo = useMemo(() => {
    const geometry = new THREE.TorusGeometry(0.2, 0.045, 6, 10);
    geometry.rotateX(Math.PI / 2);
    return geometry;
  }, []);

  useFrame((state) => {
    const body = bodies.current;
    const shell = shells.current;
    const leg = legs.current;
    const core = cores.current;
    const bolt = bolts.current;
    const ring = rings.current;
    if (!body || !shell || !leg || !core || !bolt || !ring) return;
    const time = state.clock.elapsedTime;
    CITY_AGENTS.forEach((agent, index) => {
      const pose = placeAgent(index, time, paused);
      const squash = paused ? 1 : 1 + Math.sin(time * 7 + index) * 0.06;
      writeInstance(body, index, pose.x, pose.y, pose.z, pose.rot, 1, squash, 1, agent.spec.color);
      writeInstance(shell, index, pose.x, pose.y, pose.z, pose.rot, 1.16, squash * 1.16, 1.16);
      for (const side of [-1, 1]) {
        const rx = Math.cos(pose.rot) * side * 0.34;
        const rz = -Math.sin(pose.rot) * side * 0.34;
        writeInstance(
          leg,
          index * 2 + (side < 0 ? 0 : 1),
          pose.x + rx,
          pose.y - 0.42,
          pose.z + rz,
          pose.rot,
          1,
          1,
          1,
          "#3a2414",
        );
      }
      const fx = Math.sin(pose.rot);
      const fz = Math.cos(pose.rot);
      const rx = Math.cos(pose.rot);
      const rz = -Math.sin(pose.rot);
      for (let cell = 0; cell < 4; cell += 1) {
        const on = (agent.spec.core & (1 << cell)) !== 0;
        const lx = ((cell % 2) - 0.5) * 0.16;
        const ly = (Math.floor(cell / 2) - 0.5) * 0.12;
        writeInstance(
          core,
          index * 4 + cell,
          pose.x + rx * lx + fx * 0.4,
          pose.y + ly,
          pose.z + rz * lx + fz * 0.4,
          pose.rot,
          on ? 1 : 0.4,
          on ? 1 : 0.4,
          1,
          on ? "#fff4c8" : "#6a5438",
        );
      }
      const boltScale = agent.spec.hat === "bolt" ? 1 : 0.001;
      const ringScale = agent.spec.hat === "ring" ? 1 : 0.001;
      writeInstance(bolt, index, pose.x, pose.y + 0.42 * squash, pose.z, pose.rot, boltScale, boltScale, boltScale, "#d5dde4");
      writeInstance(ring, index, pose.x, pose.y + 0.36 * squash, pose.z, pose.rot, ringScale, ringScale, ringScale, "#f2c14e");
      const tag = tags.current[index];
      if (tag) tag.position.set(pose.x, pose.y + 0.85, pose.z);
    });
    [body, shell, leg, core, bolt, ring].forEach((mesh) => {
      mesh.instanceMatrix.needsUpdate = true;
      if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
    });
  });

  return (
    <group>
      <instancedMesh ref={shells} args={[shellGeo, undefined, CITY_AGENTS.length]} frustumCulled={false}>
        <meshBasicMaterial color="#14181f" side={THREE.BackSide} />
      </instancedMesh>
      <instancedMesh
        ref={bodies}
        args={[bodyGeo, undefined, CITY_AGENTS.length]}
        frustumCulled={false}
        onClick={(event) => {
          event.stopPropagation();
          const id = event.instanceId;
          if (id === undefined || id === null) return;
          onFollow(id);
        }}
      >
        <meshToonMaterial color="#ffffff" />
      </instancedMesh>
      <instancedMesh ref={legs} args={[undefined, undefined, CITY_AGENTS.length * 2]} frustumCulled={false}>
        <boxGeometry args={[0.18, 0.2, 0.18]} />
        <meshToonMaterial color="#ffffff" />
      </instancedMesh>
      <instancedMesh ref={cores} args={[undefined, undefined, CITY_AGENTS.length * 4]} frustumCulled={false}>
        <boxGeometry args={[0.1, 0.08, 0.05]} />
        <meshBasicMaterial color="#ffffff" />
      </instancedMesh>
      <instancedMesh ref={bolts} args={[undefined, undefined, CITY_AGENTS.length]} frustumCulled={false}>
        <cylinderGeometry args={[0.1, 0.14, 0.2, 6]} />
        <meshToonMaterial color="#ffffff" />
      </instancedMesh>
      <instancedMesh ref={rings} args={[ringGeo, undefined, CITY_AGENTS.length]} frustumCulled={false}>
        <meshToonMaterial color="#ffffff" />
      </instancedMesh>
      {CITY_AGENTS.map((agent, index) => (
        <group
          key={agent.accountId}
          ref={(node) => {
            tags.current[index] = node;
          }}
        >
          <Html center position={[0, 0.15, 0]} zIndexRange={[2, 0]} style={{ pointerEvents: "none" }}>
            <div className={audit ? "name-tag audit" : "name-tag"}>{agent.name}</div>
          </Html>
        </group>
      ))}
    </group>
  );
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
      orthographic
      dpr={dpr}
      camera={{ position: [0, 18, 14], zoom: 24, near: 0.1, far: 80 }}
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
      <color attach="background" args={["#121418"]} />
      <hemisphereLight args={["#fff6ea", "#1c2128", 0.95]} />
      <directionalLight position={[6, 12, 4]} intensity={0.45} />
      <PerformanceMonitor
        flipflops={4}
        onDecline={() => setDpr(1)}
        onIncline={() => setDpr([1, 1.5])}
      />
      <mesh rotation={[-Math.PI / 2, 0, 0]} position={[0, -0.04, 0]}>
        <planeGeometry args={[42, 30]} />
        <meshToonMaterial color="#121418" />
      </mesh>
      <Rooms onPick={onPick} />
      <Agents paused={paused} audit={audit} onFollow={onFollow} />
      <ApplyToon map={gradient} />
      <CameraDriver mode={mode} beat={beat} focusId={focusId} followIndex={followIndex} paused={paused} />
      <OrbitControls
        makeDefault
        enablePan={false}
        autoRotate={mode === "orbit"}
        autoRotateSpeed={0.35}
        minPolarAngle={0.25}
        maxPolarAngle={1.2}
        target={[0, 0, 0]}
      />
    </Canvas>
  );
}
