"use client";

import { useFrame, useThree } from "@react-three/fiber";
import { useLayoutEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { RGBELoader } from "three/examples/jsm/loaders/RGBELoader.js";

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
    vec3 indigo = vec3(0.106, 0.122, 0.231);
    vec3 violet = vec3(0.290, 0.231, 0.420);
    vec3 glow = vec3(0.914, 0.518, 0.353);
    vec3 color = mix(glow, violet, smoothstep(0.0, 0.08, h));
    color = mix(color, indigo, smoothstep(0.05, 0.55, h));
    float star = step(0.997, hash(floor(dir * 180.0)));
    color += vec3(0.85, 0.88, 1.0) * star * smoothstep(0.15, 0.4, h);
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
      <sphereGeometry args={[320, 32, 24]} />
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
    loader.load("/textures/kiara_1_dawn_1k.hdr", (texture) => {
      if (!alive) {
        texture.dispose();
        return;
      }
      texture.mapping = THREE.EquirectangularReflectionMapping;
      const pmrem = new THREE.PMREMGenerator(gl);
      env = pmrem.fromEquirectangular(texture).texture;
      sky = texture;
      scene.environment = env;
      scene.environmentIntensity = 0.18;
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
      <meshStandardMaterial color="#1d3a32" roughness={0.94} metalness={0} envMapIntensity={0.02} />
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
  return (
    <group>
      {LAMP_SPOTS.map(([x, z], index) => (
        <group key={`${x}-${z}`} position={[x, 0, z]}>
          <mesh position={[0, 2.6, 0]}>
            <cylinderGeometry args={[0.06, 0.08, 5.2, 6]} />
            <meshStandardMaterial color="#2a241c" roughness={0.7} metalness={0.3} />
          </mesh>
          <mesh position={[0, 5.3, 0]}>
            <sphereGeometry args={[0.22, 8, 6]} />
            <meshStandardMaterial color="#ffd7a0" emissive="#ffb15a" emissiveIntensity={2.4} roughness={0.4} />
          </mesh>
          <mesh rotation={[-Math.PI / 2, 0, 0]} position={[0, 0.05, 0]}>
            <circleGeometry args={[2.4, 16]} />
            <meshBasicMaterial color="#ffb56a" transparent opacity={0.22} depthWrite={false} />
          </mesh>
          {index < 6 ? (
            <pointLight position={[0, 5.1, 0]} color="#ffb060" intensity={7} distance={16} decay={2} />
          ) : null}
        </group>
      ))}
    </group>
  );
}

function GroundMist() {
  return (
    <mesh rotation={[-Math.PI / 2, 0, 0]} position={[0, 0.35, 0]}>
      <circleGeometry args={[78, 24]} />
      <meshBasicMaterial color="#3a4468" transparent opacity={0.18} depthWrite={false} />
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

export function World() {
  return (
    <>
      <Sky />
      <EnvironmentLight />
      <GrassBlades />
      <StreetLamps />
      <GroundMist />
      <Fireflies />
      <hemisphereLight args={["#8ea0d4", "#2a3348", 0.38]} />
      <ambientLight intensity={0.16} color="#3a4468" />
      <directionalLight
        position={[78, 9, 18]}
        intensity={0.4}
        color="#ffb07a"
        castShadow
        shadow-mapSize={[2048, 2048]}
        shadow-bias={-0.001}
        shadow-normalBias={0.12}
        shadow-camera-near={8}
        shadow-camera-far={320}
        shadow-camera-left={-120}
        shadow-camera-right={120}
        shadow-camera-top={120}
        shadow-camera-bottom={-120}
      />
      <directionalLight position={[-40, 16, -36]} intensity={0.32} color="#8eaaee" />
      <fog attach="fog" args={["#2a2448", 220, 640]} />
    </>
  );
}
