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
    float glow = pow(max(dot(dir, sun), 0.0), 28.0);
    color += vec3(1.0, 0.72, 0.42) * glow * 0.85;
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
    loader.load("/textures/kiara_1_dawn_1k.hdr", (texture) => {
      if (!alive) {
        texture.dispose();
        return;
      }
      texture.mapping = THREE.EquirectangularReflectionMapping;
      const pmrem = new THREE.PMREMGenerator(gl);
      env = pmrem.fromEquirectangular(texture).texture;
      scene.environment = env;
      texture.dispose();
      pmrem.dispose();
    });
    return () => {
      alive = false;
      if (scene.environment === env) scene.environment = null;
      env?.dispose();
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
    [-40, -3.2, -20, 46, 18],
    [-10, -4.4, 10, 54, 20],
    [24, -2.6, -30, 40, 16],
    [50, -5.5, 6, 60, 22],
    [-55, -6.2, 24, 50, 18],
    [8, -7.4, 36, 70, 24],
    [-28, -3.8, 40, 36, 14],
  ] as const;
  return (
    <group ref={group}>
      {puffs.map(([x, y, z, w, h], index) => (
        <mesh key={index} position={[x, y, z]} rotation={[-Math.PI / 2, 0, index * 0.2]}>
          <planeGeometry args={[w, h]} />
          <meshBasicMaterial map={texture} transparent depthWrite={false} fog />
        </mesh>
      ))}
    </group>
  );
}

function Islands() {
  const islands = [
    [-70, 6, -40, 10, 16],
    [80, 2, -55, 8, 12],
    [-90, 10, 30, 7, 18],
    [60, 14, 70, 6, 14],
    [-40, 4, 95, 9, 11],
    [100, 8, 10, 5, 20],
  ] as const;
  return (
    <group>
      {islands.map(([x, y, z, radius, height], index) => (
        <mesh key={index} position={[x, y, z]} castShadow>
          <cylinderGeometry args={[radius * 0.45, radius, height, 6]} />
          <meshStandardMaterial color={index % 2 === 0 ? "#c7b4dc" : "#f0c7a4"} roughness={0.85} metalness={0.05} />
        </mesh>
      ))}
    </group>
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
            opacity={0.09}
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
      <Sky />
      <EnvironmentLight />
      <CloudSea />
      <Islands />
      <SunBeams />
      <hemisphereLight args={["#ffe0c4", "#b9a3e8", 0.32]} />
      <ambientLight intensity={0.16} />
      <directionalLight
        position={[28, 42, 14]}
        intensity={1.7}
        color="#ffd2a8"
        castShadow
        shadow-mapSize={[2048, 2048]}
        shadow-bias={-0.0004}
        shadow-normalBias={0.04}
        shadow-camera-near={2}
        shadow-camera-far={90}
        shadow-camera-left={-36}
        shadow-camera-right={36}
        shadow-camera-top={36}
        shadow-camera-bottom={-36}
      />
      <fog attach="fog" args={["#f3c7a4", 95, 260]} />
    </>
  );
}
