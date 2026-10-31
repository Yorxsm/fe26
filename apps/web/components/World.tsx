"use client";

import { useFrame, useThree } from "@react-three/fiber";
import { useLayoutEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { RGBELoader } from "three/examples/jsm/loaders/RGBELoader.js";
import type { QualityTier } from "@/lib/city";

const skyVertex = `
  varying vec3 vPos;
  void main() {
    vec4 world = modelMatrix * vec4(position, 1.0);
    vPos = world.xyz;
    gl_Position = projectionMatrix * viewMatrix * world;
  }
`;

const skyFragment = `
  varying vec3 vPos;
  float hash(vec3 p) {
    return fract(sin(dot(p, vec3(127.1, 311.7, 74.7))) * 43758.5453);
  }
  void main() {
    vec3 dir = normalize(vPos);
    float h = clamp(dir.y, 0.0, 1.0);
    vec3 deep = vec3(0.027, 0.039, 0.102);
    vec3 indigo = vec3(0.059, 0.071, 0.157);
    vec3 violet = vec3(0.133, 0.102, 0.180);
    vec3 glow = vec3(0.38, 0.16, 0.10);
    vec3 color = mix(glow, violet, smoothstep(0.0, 0.04, h));
    color = mix(color, indigo, smoothstep(0.025, 0.20, h));
    color = mix(color, deep, smoothstep(0.16, 0.55, h));
    if (dir.y < 0.0) color = mix(vec3(0.04, 0.045, 0.055), glow, smoothstep(-0.12, 0.0, dir.y));
    float star = step(0.9975, hash(floor(dir * 220.0)));
    color += vec3(0.75, 0.82, 0.95) * star * smoothstep(0.28, 0.5, h);
    gl_FragColor = vec4(color, 1.0);
  }
`;

function Sky() {
  const material = useMemo(
    () =>
      new THREE.ShaderMaterial({
        vertexShader: skyVertex,
        fragmentShader: skyFragment,
        side: THREE.BackSide,
        depthWrite: false,
        fog: false,
      }),
    [],
  );
  return (
    <mesh frustumCulled={false} material={material}>
      <sphereGeometry args={[1400, 32, 24]} />
    </mesh>
  );
}

function EnvironmentLight() {
  const gl = useThree((state) => state.gl);
  const scene = useThree((state) => state.scene);
  useLayoutEffect(() => {
    const loader = new RGBELoader();
    let alive = true;
    let env: THREE.Texture | null = null;
    let sky: THREE.Texture | null = null;
    loader.load("/textures/kiara_1_dawn_512.hdr", (texture) => {
      if (!alive) {
        texture.dispose();
        return;
      }
      texture.mapping = THREE.EquirectangularReflectionMapping;
      const pmrem = new THREE.PMREMGenerator(gl);
      env = pmrem.fromEquirectangular(texture).texture;
      sky = texture;
      scene.environment = env;
      scene.environmentIntensity = 0.3;
      pmrem.dispose();
    });
    return () => {
      alive = false;
      if (scene.environment === env) scene.environment = null;
      if (scene.background === sky) scene.background = null;
      env?.dispose();
      sky?.dispose();
    };
  }, [gl, scene]);
  return null;
}

function GrassBlades() {
  const mesh = useRef<THREE.InstancedMesh>(null);
  useLayoutEffect(() => {
    const blade = mesh.current;
    if (!blade) return;
    const dummy = new THREE.Object3D();
    for (let index = 0; index < 700; index += 1) {
      const angle = index * 2.399;
      const radius = 4 + (index % 28);
      dummy.position.set(Math.cos(angle) * radius, 0.25, Math.sin(angle) * radius);
      dummy.rotation.y = angle;
      dummy.scale.set(0.12, 0.35 + (index % 5) * 0.08, 0.12);
      dummy.updateMatrix();
      blade.setMatrixAt(index, dummy.matrix);
    }
    blade.instanceMatrix.needsUpdate = true;
  }, []);
  useFrame(({ clock }) => {
    const blade = mesh.current;
    if (!blade) return;
    blade.rotation.z = Math.sin(clock.elapsedTime * 0.6) * 0.04;
  });
  return (
    <instancedMesh ref={mesh} args={[undefined, undefined, 700]}>
      <coneGeometry args={[0.08, 0.7, 3]} />
      <meshStandardMaterial color="#3e6a50" emissive="#24543c" emissiveIntensity={0.7} roughness={0.94} metalness={0} envMapIntensity={0.02} />
    </instancedMesh>
  );
}

const LAMP_SPOTS: Array<[number, number]> = [
  [-50, 6],
  [-28, -8],
  [-8, -22],
  [18, -18],
  [38, -4],
  [48, 22],
  [24, 36],
  [2, 28],
  [-22, 30],
  [-40, 28],
  [8, 6],
  [22, 8],
];

function StreetLamps() {
  const poles = useRef<THREE.InstancedMesh>(null);
  const heads = useRef<THREE.InstancedMesh>(null);
  const pools = useRef<THREE.InstancedMesh>(null);
  useLayoutEffect(() => {
    const dummy = new THREE.Object3D();
    const write = (field: THREE.InstancedMesh | null, y: number, rotX: number) => {
      if (!field) return;
      LAMP_SPOTS.forEach(([x, z], index) => {
        dummy.position.set(x, y, z);
        dummy.rotation.set(rotX, 0, 0);
        dummy.scale.set(1, 1, 1);
        dummy.updateMatrix();
        field.setMatrixAt(index, dummy.matrix);
      });
      field.instanceMatrix.needsUpdate = true;
    };
    write(poles.current, 2.6, 0);
    write(heads.current, 5.3, 0);
    write(pools.current, 0.05, -Math.PI / 2);
  }, []);
  return (
    <group>
      <instancedMesh ref={poles} args={[undefined, undefined, LAMP_SPOTS.length]} castShadow receiveShadow>
        <cylinderGeometry args={[0.06, 0.08, 5.2, 6]} />
        <meshStandardMaterial color="#2a241c" roughness={0.7} metalness={0.3} />
      </instancedMesh>
      <instancedMesh ref={heads} args={[undefined, undefined, LAMP_SPOTS.length]} castShadow>
        <sphereGeometry args={[0.22, 8, 6]} />
        <meshStandardMaterial color="#ffd7a0" emissive="#ffb15a" emissiveIntensity={0.55} roughness={0.45} />
      </instancedMesh>
      <instancedMesh ref={pools} args={[undefined, undefined, LAMP_SPOTS.length]}>
        <circleGeometry args={[5.2, 20]} />
        <meshBasicMaterial color="#ffb56a" transparent opacity={0.38} depthWrite={false} />
      </instancedMesh>
      {LAMP_SPOTS.map(([x, z]) => (
        <pointLight key={`${x}-${z}`} position={[x, 4.2, z]} color="#ffb060" intensity={12} distance={28} decay={2} />
      ))}
    </group>
  );
}

function GroundMist() {
  return (
    <mesh rotation={[-Math.PI / 2, 0, 0]} position={[0, 0.4, 0]}>
      <circleGeometry args={[220, 40]} />
      <meshBasicMaterial color="#4a3424" transparent opacity={0.08} depthWrite={false} />
    </mesh>
  );
}

function Fireflies() {
  const mesh = useRef<THREE.InstancedMesh>(null);
  const seeds = useMemo(
    () =>
      Array.from({ length: 48 }, (_, index) => ({
        x: Math.cos(index * 2.4) * (12 + (index % 9) * 4),
        z: Math.sin(index * 2.4) * (12 + (index % 7) * 5),
        y: 1.2 + (index % 5) * 0.45,
        phase: index * 0.7,
      })),
    [],
  );
  useFrame(({ clock }) => {
    const field = mesh.current;
    if (!field) return;
    const dummy = new THREE.Object3D();
    seeds.forEach((seed, index) => {
      const t = clock.elapsedTime;
      dummy.position.set(seed.x + Math.sin(t * 0.6 + seed.phase) * 0.6, seed.y + Math.sin(t * 1.3 + seed.phase) * 0.35, seed.z);
      const twinkle = 0.4 + Math.sin(t * 3 + seed.phase) * 0.35;
      dummy.scale.setScalar(Math.max(0.15, twinkle));
      dummy.updateMatrix();
      field.setMatrixAt(index, dummy.matrix);
    });
    field.instanceMatrix.needsUpdate = true;
  });
  return (
    <instancedMesh ref={mesh} args={[undefined, undefined, 48]}>
      <sphereGeometry args={[0.08, 6, 4]} />
      <meshBasicMaterial color="#d6ff9a" transparent opacity={0.85} depthWrite={false} />
    </instancedMesh>
  );
}

export function World({ quality = "medium" }: { quality?: QualityTier }) {
  const shadowSize = quality === "high" ? 2048 : 1024;
  const shadows = quality !== "low";
  return (
    <>
      <Sky />
      <EnvironmentLight />
      <GrassBlades />
      <StreetLamps />
      <GroundMist />
      <Fireflies />
      <hemisphereLight args={["#d2c4ae", "#5a4c3c", 0.72]} />
      <ambientLight intensity={0.45} color="#74604c" />
      <directionalLight
        position={[78, 9, 18]}
        intensity={0.38}
        color="#ffb07a"
        castShadow={shadows}
        shadow-mapSize={[shadowSize, shadowSize]}
        shadow-bias={-0.0008}
        shadow-normalBias={0.08}
        shadow-camera-near={1}
        shadow-camera-far={240}
        shadow-camera-left={-86}
        shadow-camera-right={86}
        shadow-camera-top={86}
        shadow-camera-bottom={-86}
      />
      <directionalLight position={[-32, 24, 58]} intensity={1.15} color="#ffc49a" />
      <pointLight position={[-19, 5, 32]} color="#ffc48a" intensity={32} distance={26} decay={1} />
      <pointLight position={[-28, 5, 44]} color="#ffb060" intensity={22} distance={58} decay={1} />
      <pointLight position={[-8, 4.4, 40]} color="#ffc48a" intensity={22} distance={56} decay={1} />
      <pointLight position={[10, 4.2, 32]} color="#ffb070" intensity={20} distance={52} decay={1} />
      <pointLight position={[4, 4, 14]} color="#ffb060" intensity={16} distance={44} decay={1} />
      <pointLight position={[-6, 3.8, 8]} color="#ffc48a" intensity={14} distance={40} decay={1} />
      <directionalLight position={[-18, 22, -64]} intensity={1.05} color="#e0d0b8" />
      <pointLight position={[12, 5.5, 52]} color="#ffb070" intensity={420} distance={9} decay={2} />
      <pointLight position={[-44, 3.2, -14]} color="#ffb060" intensity={500} distance={7} decay={2} />
      <pointLight position={[-38, 3.2, -14]} color="#ffb060" intensity={440} distance={5.2} decay={2} />
      <pointLight position={[-32, 3.2, -14]} color="#ffb060" intensity={500} distance={7} decay={2} />
      <pointLight position={[-30, 3.2, -30]} color="#ffb060" intensity={1200} distance={14} decay={2} />
      <pointLight position={[-18, 3.2, -26]} color="#ffb060" intensity={1200} distance={14} decay={2} />
      <pointLight position={[-8, 3.2, -24]} color="#ffb060" intensity={1100} distance={12} decay={2} />
      <pointLight position={[6, 3.4, -32]} color="#ffb060" intensity={1200} distance={13} decay={2} />
      <fog attach="fog" args={["#100e18", 70, 280]} />
    </>
  );
}
