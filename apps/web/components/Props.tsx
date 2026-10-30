"use client";

import { useGLTF } from "@react-three/drei";
import { useMemo } from "react";
import * as THREE from "three";

const BENCH = "/models/props/painted_wooden_bench.glb";
const PLANTER_A = "/models/props/planter_box_01.glb";
const PLANTER_B = "/models/props/planter_box_02.glb";
const CRATE = "/models/props/wooden_crate_01.glb";
const BARRELS = "/models/props/wooden_barrels_01.glb";
const CART = "/models/props/CoffeeCart_01.glb";
const TABLE = "/models/props/dining_table.glb";
const BASKET = "/models/props/wicker_basket_01.glb";
const APPLE = "/models/props/food_apple_01.glb";

for (const url of [BENCH, PLANTER_A, PLANTER_B, CRATE, BARRELS, CART, TABLE, BASKET, APPLE]) {
  useGLTF.preload(url, false, true);
}

type Vec3 = [number, number, number];

function Prop({
  url,
  position,
  rotation = 0,
  scale = 1,
}: {
  url: string;
  position: Vec3;
  rotation?: number;
  scale?: number | Vec3;
}) {
  const { scene } = useGLTF(url, false, true);
  const object = useMemo(() => {
    const clone = scene.clone(true);
    clone.traverse((node) => {
      const mesh = node as THREE.Mesh;
      if (!mesh.isMesh) return;
      mesh.castShadow = true;
      mesh.receiveShadow = true;
      mesh.raycast = () => undefined;
      const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
      for (const material of materials) {
        if ((material as THREE.MeshStandardMaterial).isMeshStandardMaterial) {
          (material as THREE.MeshStandardMaterial).envMapIntensity = 0.55;
        }
      }
    });
    return clone;
  }, [scene]);
  return <primitive object={object} position={position} rotation={[0, rotation, 0]} scale={scale} />;
}

function Stall({ x, z, rot = 0 }: { x: number; z: number; rot?: number }) {
  return (
    <group position={[x, 0, z]} rotation={[0, rot, 0]}>
      <Prop url={TABLE} position={[0, 0, 0]} />
      <Prop url={CRATE} position={[-1.35, 0, 0.55]} rotation={0.18} />
      <Prop url={CRATE} position={[1.42, 0, 0.5]} rotation={-0.22} scale={0.9} />
      <Prop url={BASKET} position={[-0.62, 0.89, 0.05]} rotation={0.4} />
      <Prop url={APPLE} position={[-0.55, 0.96, -0.02]} rotation={0.2} />
      <Prop url={APPLE} position={[-0.7, 0.96, 0.14]} rotation={1.1} />
      <Prop url={APPLE} position={[-0.45, 0.96, 0.15]} rotation={2.2} />
      <Prop url={BASKET} position={[0.72, 0.89, 0.08]} rotation={-0.5} scale={0.9} />
      <Prop url={APPLE} position={[0.78, 0.95, 0.04]} rotation={0.8} />
      <Prop url={APPLE} position={[0.63, 0.95, 0.14]} rotation={2.6} />
      {[
        [-1.35, -0.82],
        [1.35, -0.82],
        [-1.35, 0.82],
        [1.35, 0.82],
      ].map(([px, pz]) => (
        <mesh key={`${px}-${pz}`} position={[px, 1.18, pz]} castShadow>
          <cylinderGeometry args={[0.035, 0.045, 2.36, 6]} />
          <meshStandardMaterial color="#5b3a25" roughness={0.86} metalness={0.04} />
        </mesh>
      ))}
      <mesh position={[0, 2.38, 0]} rotation={[0.06, 0, 0]} castShadow receiveShadow>
        <boxGeometry args={[3.0, 0.07, 1.92]} />
        <meshStandardMaterial color="#b64a29" roughness={0.94} metalness={0} />
      </mesh>
      <mesh position={[0, 2.17, 0.95]} castShadow>
        <boxGeometry args={[3.0, 0.34, 0.045]} />
        <meshStandardMaterial color="#8e321d" roughness={0.96} metalness={0} side={THREE.DoubleSide} />
      </mesh>
      <mesh position={[0, 2.17, -0.95]} castShadow>
        <boxGeometry args={[3.0, 0.34, 0.045]} />
        <meshStandardMaterial color="#8e321d" roughness={0.96} metalness={0} side={THREE.DoubleSide} />
      </mesh>
      {[-0.9, 0, 0.9].map((bx) => (
        <mesh key={bx} position={[bx, 2.24, 0.8]}>
          <sphereGeometry args={[0.055, 8, 6]} />
          <meshStandardMaterial color="#ffd7a0" emissive="#ff9d3d" emissiveIntensity={1.6} />
        </mesh>
      ))}
    </group>
  );
}

export function Props() {
  if (typeof window !== "undefined" && new URLSearchParams(window.location.search).get("props") === "0") {
    return null;
  }
  return (
    <group>
      <Stall x={38} z={26.6} />
      <Stall x={42.6} z={26.6} rot={0.03} />
      <Stall x={50.2} z={26.6} rot={-0.03} />
      <Stall x={54.8} z={26.6} />

      <Prop url={CART} position={[58.4, 0, 20.4]} rotation={-0.62} />
      <Prop url={CART} position={[-34.2, 0, -12.6]} rotation={0.45} />
      <Prop url={BARRELS} position={[34.6, 0, 25.3]} rotation={0.35} />
      <Prop url={BARRELS} position={[18.2, 0, 50.2]} rotation={-0.3} scale={0.9} />

      {[
        [36.2, 23.1, Math.PI],
        [55.6, 23.1, Math.PI],
        [-47.2, 27.8, 0],
        [-36.9, 27.8, 0],
        [6.8, 49.4, Math.PI],
        [17.2, 49.4, Math.PI],
        [-13.2, -31.4, Math.PI],
        [-2.8, -31.4, Math.PI],
        [-43.4, -13.4, 0],
        [-32.6, -13.4, 0],
      ].map(([x, z, rot]) => (
        <Prop key={`${x}-${z}`} url={BENCH} position={[x, 0, z]} rotation={rot} />
      ))}

      {[
        [-44.4, 27.1, 0.2, PLANTER_A],
        [-39.6, 27.1, -0.15, PLANTER_B],
        [43.2, 23.6, 0.1, PLANTER_B],
        [48.8, 23.6, -0.12, PLANTER_A],
        [9.2, 49.6, 0.15, PLANTER_A],
        [14.8, 49.6, -0.2, PLANTER_B],
        [-11.2, -30.4, 0.1, PLANTER_B],
        [-4.8, -30.4, -0.1, PLANTER_A],
        [-40.4, -13.7, 0.18, PLANTER_A],
        [-35.6, -13.7, -0.15, PLANTER_B],
        [34.8, 27.8, 0.4, PLANTER_B],
        [57.2, 27.4, -0.35, PLANTER_A],
      ].map(([x, z, rot, url]) => (
        <Prop key={`${x}-${z}-${url}`} url={url as string} position={[x as number, 0, z as number]} rotation={rot as number} />
      ))}

      {[
        [-47.4, 28.2, 0.3],
        [-46.4, 28.8, -0.2],
        [18.6, 50.4, 0.25],
        [-14.4, -29.8, -0.3],
        [-33.8, -12.6, 0.15],
        [35.4, 24.2, -0.4],
      ].map(([x, z, rot]) => (
        <Prop key={`${x}-${z}-crate`} url={CRATE} position={[x, 0, z]} rotation={rot} />
      ))}
    </group>
  );
}
