"use client";

import { OrbitControls, PerformanceMonitor, useGLTF } from "@react-three/drei";
import { Canvas, useFrame } from "@react-three/fiber";
import { useLayoutEffect, useMemo, useState } from "react";
import * as THREE from "three";
import { SkeletonUtils } from "three-stdlib";
import { ROLE_COLORS, expressionVisible, roleVisible, type Mood, type SkinTraits } from "@/lib/skin";
import type { Object3D } from "three";

function sourceName(node: Object3D): string {
  let current: Object3D | null = node;
  while (current) {
    const stored = current.userData.name;
    if (typeof stored === "string" && stored.includes(".")) return stored;
    current = current.parent;
  }
  return node.name;
}

useGLTF.preload("/models/agent.glb", false, true);

function PreviewMesh({ traits, mood }: { traits: SkinTraits; mood: Mood }) {
  const gltf = useGLTF("/models/agent.glb", false, true);
  const scene = useMemo(() => SkeletonUtils.clone(gltf.scene), [gltf.scene]);
  const mixer = useMemo(() => new THREE.AnimationMixer(scene), [scene]);

  useLayoutEffect(() => {
    const clip = gltf.animations.find((entry) => entry.name === "idle");
    if (!clip) return;
    mixer.stopAllAction();
    mixer.clipAction(clip).play();
  }, [gltf.animations, mixer]);

  useLayoutEffect(() => {
    const color = ROLE_COLORS[traits.role] ?? ROLE_COLORS[0];
    scene.traverse((node) => {
      const label = sourceName(node);
      const shown = roleVisible(label, traits, mood) ?? expressionVisible(label, traits, mood);
      if (shown !== null) node.visible = shown;
      const mesh = node as THREE.Mesh;
      if (!mesh.isMesh) return;
      const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
      materials.forEach((material) => {
        const standard = material as THREE.MeshStandardMaterial;
        if (standard.name !== "Body" || !standard.color) return;
        standard.color.set(color);
      });
    });
  }, [mood, scene, traits]);

  useFrame((_, delta) => mixer.update(delta));

  return <primitive object={scene} />;
}

export function SkinPreview({ traits, mood = "trait" }: { traits: SkinTraits; mood?: Mood }) {
  const [dpr, setDpr] = useState<number | [number, number]>([1, 1.5]);
  const [ready, setReady] = useState(true);

  if (!ready) return <p className="meta">WebGL is unavailable in this browser.</p>;

  return (
    <Canvas
      className="forge-canvas"
      dpr={dpr}
      camera={{ position: [0, 1.15, 2.4], fov: 32, near: 0.1, far: 20 }}
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
          setReady(false);
          setDpr(1);
        });
      }}
    >
      <color attach="background" args={["#0a0a0b"]} />
      <hemisphereLight args={["#8ea0d4", "#1c140e", 0.4]} />
      <directionalLight position={[2, 1.2, 2]} intensity={0.55} color="#ffb07a" />
      <PerformanceMonitor flipflops={4} onDecline={() => setDpr(1)} onIncline={() => setDpr([1, 1.5])} />
      <PreviewMesh traits={traits} mood={mood} />
      <OrbitControls enablePan={false} minDistance={1.4} maxDistance={4} target={[0, 0.8, 0]} />
    </Canvas>
  );
}
