"use client";

import { OrbitControls, PerformanceMonitor, useGLTF } from "@react-three/drei";
import { Canvas, useFrame } from "@react-three/fiber";
import { useLayoutEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import { SkeletonUtils } from "three-stdlib";
import { mountFace, redrawFace } from "@/lib/faceCanvas";
import { nodeVisible, paintFinish, screenColor, type Mood, type SkinTraits } from "@/lib/skin";
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
    const finish = paintFinish(traits.paint);
    const glow = screenColor(traits.screen);
    scene.traverse((node) => {
      const label = sourceName(node);
      const shown = nodeVisible(label, traits, mood);
      if (shown !== null) node.visible = shown;
      const mesh = node as THREE.Mesh;
      if (!mesh.isMesh) return;
      const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
      materials.forEach((material) => {
        const standard = material as THREE.MeshStandardMaterial;
        if (!standard.color) return;
        if (standard.name === "Body") {
          standard.color.set(finish.color);
          standard.metalness = finish.metal;
          standard.roughness = finish.rough;
        }
        if (standard.name === "Screen") {
          standard.color.set("#0a0a0b");
          standard.emissive.set("#000000");
          standard.emissiveIntensity = 0;
          standard.metalness = 0.62;
          standard.roughness = 0.14;
        }
      });
    });
  }, [mood, scene, traits]);

  const face = useMemo(() => mountFace(scene, sourceName), [scene]);
  const blinkAt = useRef(2.2);

  useFrame((state, delta) => {
    mixer.update(delta);
    if (!face) return;
    const now = state.clock.elapsedTime;
    let shut = 0;
    if (now > blinkAt.current) blinkAt.current = now + 3.4;
    const into = blinkAt.current - now;
    if (into > 3.2) shut = Math.sin((3.4 - into) * 22);
    const camera = state.camera.position;
    face.time.value = now;
    redrawFace(face, {
      style: traits.eyes,
      expression: mood === "happy" ? "happy" : mood === "alarm" ? "shocked" : "calm",
      blink: Math.max(0, Math.min(1, shut)),
      lookX: Math.max(-1, Math.min(1, camera.x * 0.8)),
      lookY: Math.max(-1, Math.min(1, (camera.y - 0.7) * 0.6)),
      color: screenColor(traits.screen),
    });
  });

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
      camera={{ position: [1.15, 0.62, 3.15], fov: 26, near: 0.08, far: 24 }}
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
      <hemisphereLight args={["#8ea0d4", "#2a2018", 0.55]} />
      <ambientLight intensity={0.2} color="#4a4036" />
      <directionalLight position={[2.2, 2.4, 2.4]} intensity={1.1} color="#ffc49a" />
      <pointLight position={[0.4, 1.2, 1.4]} color="#ffb060" intensity={6} distance={6} decay={2} />
      <PerformanceMonitor flipflops={4} onDecline={() => setDpr(1)} onIncline={() => setDpr([1, 1.5])} />
      <PreviewMesh traits={traits} mood={mood} />
      <OrbitControls autoRotate autoRotateSpeed={0.55} enablePan={false} enableRotate minDistance={2.1} maxDistance={4.8} target={[0, 0.52, 0]} />
    </Canvas>
  );
}
