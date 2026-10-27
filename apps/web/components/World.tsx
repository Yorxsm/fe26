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
  void main() {
    vec3 dir = normalize(vPos);
    float h = dir.y * 0.5 + 0.5;
    vec3 peach = vec3(1.0, 0.811, 0.62);
    vec3 lilac = vec3(0.725, 0.639, 0.91);
    vec3 blue = vec3(0.561, 0.722, 1.0);
    vec3 color = mix(peach, lilac, smoothstep(0.02, 0.48, h));
    color = mix(color, blue, smoothstep(0.42, 0.95, h));
    vec3 sun = normalize(vec3(0.55, 0.28, 0.22));
    float glow = pow(max(dot(dir, sun), 0.0), 48.0);
    color += vec3(1.0, 0.78, 0.45) * glow * 0.28;
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
      scene.background = texture;
      scene.backgroundBlurriness = 0.12;
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

function CloudSea() {
  const group = useRef<THREE.Group>(null);
  const texture = useMemo(() => {
    const map = new THREE.TextureLoader().load("/textures/cloud.png");
    map.colorSpace = THREE.SRGBColorSpace;
    return map;
  }, []);
  useFrame((_, delta) => {
    const root = group.current;
    if (!root) return;
    root.children.forEach((child, index) => {
      child.position.x += delta * (0.35 + index * 0.08);
      if (child.position.x > 90) child.position.x = -90;
    });
  });
  const puffs = [
    [-80, 55, -30, 90, 28],
    [-20, 70, 20, 110, 32],
    [40, 48, -50, 80, 24],
    [90, 62, 10, 100, 30],
    [-90, 40, 40, 70, 22],
    [10, 80, 70, 120, 34],
  ] as const;
  return (
    <group ref={group}>
      {puffs.map(([x, y, z, w, h], index) => (
        <mesh key={index} position={[x, y, z]} rotation={[-Math.PI / 2, 0, index * 0.2]}>
          <planeGeometry args={[w, h]} />
          <meshBasicMaterial map={texture} transparent opacity={0.4} depthWrite={false} fog />
        </mesh>
      ))}
    </group>
  );
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
      <meshStandardMaterial color="#1f7a32" roughness={0.92} metalness={0} envMapIntensity={0.02} />
    </instancedMesh>
  );
}

function SunBeams() {
  const beams = [
    [22, 28, 10, -1.05, 0.35, 0.15],
    [8, 32, 24, -1.15, 0.1, -0.2],
    [30, 24, -6, -0.9, 0.55, 0.25],
  ] as const;
  return (
    <group>
      {beams.map(([x, y, z, rx, ry, rz], index) => (
        <mesh key={index} position={[x, y, z]} rotation={[rx, ry, rz]}>
          <planeGeometry args={[7, 46]} />
          <meshBasicMaterial
            color="#ffe0b0"
            transparent
            opacity={0.035}
            depthWrite={false}
            blending={THREE.AdditiveBlending}
            fog={false}
            side={THREE.DoubleSide}
          />
        </mesh>
      ))}
    </group>
  );
}

export function World() {
  return (
    <>
      <EnvironmentLight />
      <CloudSea />
      <GrassBlades />
      <SunBeams />
      <hemisphereLight args={["#fff4dd", "#2f6a32", 0.42]} />
      <ambientLight intensity={0.22} color="#fff8ee" />
      <directionalLight
        position={[60, 80, 30]}
        intensity={1.05}
        color="#fff0d0"
        castShadow
        shadow-mapSize={[2048, 2048]}
        shadow-bias={-0.0008}
        shadow-normalBias={0.08}
        shadow-camera-near={10}
        shadow-camera-far={280}
        shadow-camera-left={-90}
        shadow-camera-right={90}
        shadow-camera-top={90}
        shadow-camera-bottom={-90}
      />
      <fog attach="fog" args={["#d5e4c4", 380, 980]} />
    </>
  );
}
