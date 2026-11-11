import * as THREE from "three";
import { GLTFLoader, type GLTF } from "three/addons/loaders/GLTFLoader.js";
import { MeshoptDecoder, SkeletonUtils } from "three-stdlib";
import { mountFace, redrawFace } from "@/lib/faceCanvas";
import { nodeVisible, paintFinish, screenColor, type SkinTraits } from "@/lib/skin";
import type { Object3D } from "three";

const WIDTH = 480;
const HEIGHT = 640;

const cache = new Map<string, string>();
const inflight = new Map<string, Promise<string>>();
let chain: Promise<void> = Promise.resolve();
let model: Promise<GLTF> | null = null;
let renderer: THREE.WebGLRenderer | null = null;

function portraitKey(traits: SkinTraits): string {
  return [
    traits.paint,
    traits.screen,
    traits.eyes,
    traits.headwear,
    traits.face_item,
    traits.back_item,
    traits.emblem,
    traits.trail,
    traits.role,
  ].join(".");
}

function sourceName(node: Object3D): string {
  let current: Object3D | null = node;
  while (current) {
    const stored = current.userData.name;
    if (typeof stored === "string" && stored.includes(".")) return stored;
    current = current.parent;
  }
  return node.name;
}

function loadModel(): Promise<GLTF> {
  if (!model) {
    model = (async () => {
      const decoder = MeshoptDecoder();
      if ("ready" in decoder) await decoder.ready;
      const loader = new GLTFLoader();
      loader.setMeshoptDecoder(decoder as Parameters<GLTFLoader["setMeshoptDecoder"]>[0]);
      return await new Promise<GLTF>((resolve, reject) => {
        loader.load("/models/agent.glb", resolve, undefined, (error) => {
          model = null;
          reject(error);
        });
      });
    })();
  }
  return model;
}

function sharedRenderer(): THREE.WebGLRenderer {
  if (renderer && !renderer.getContext().isContextLost()) return renderer;
  renderer?.dispose();
  const canvas = document.createElement("canvas");
  canvas.width = WIDTH;
  canvas.height = HEIGHT;
  renderer = new THREE.WebGLRenderer({
    canvas,
    antialias: false,
    alpha: false,
    preserveDrawingBuffer: true,
    powerPreference: "default",
  });
  renderer.setPixelRatio(1);
  renderer.setSize(WIDTH, HEIGHT, false);
  renderer.setClearColor(0x0a0a0b, 1);
  return renderer;
}

function fitCamera(camera: THREE.PerspectiveCamera, root: Object3D) {
  const box = new THREE.Box3();
  let found = false;
  root.updateWorldMatrix(true, true);
  root.traverse((node) => {
    const mesh = node as THREE.Mesh;
    if (!mesh.isMesh || !mesh.visible) return;
    box.expandByObject(mesh);
    found = true;
  });
  if (!found || box.isEmpty()) return;
  const center = box.getCenter(new THREE.Vector3());
  const size = box.getSize(new THREE.Vector3());
  const fov = (camera.fov * Math.PI) / 180;
  const aspect = WIDTH / HEIGHT;
  const distY = size.y / 2 / Math.tan(fov / 2);
  const distX = size.x / 2 / Math.tan(fov / 2) / Math.max(aspect, 0.2);
  const dist = Math.max(distY, distX, 0.4) * 1.28;
  camera.position.set(center.x, center.y, center.z + dist);
  camera.near = Math.max(0.01, dist / 80);
  camera.far = dist * 24;
  camera.lookAt(center);
  camera.updateProjectionMatrix();
}

function paint(root: Object3D, traits: SkinTraits) {
  const finish = paintFinish(traits.paint);
  root.traverse((node) => {
    const label = sourceName(node);
    const shown = nodeVisible(label, traits, "trait");
    if (shown !== null) node.visible = shown;
    const mesh = node as THREE.Mesh;
    if (!mesh.isMesh) return;
    const source = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
    const cloned = source.map((material) => material.clone());
    mesh.material = Array.isArray(mesh.material) ? cloned : cloned[0];
    cloned.forEach((material) => {
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
}

function release(root: Object3D) {
  root.traverse((node) => {
    const mesh = node as THREE.Mesh;
    if (!mesh.isMesh) return;
    const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
    materials.forEach((material) => {
      const standard = material as THREE.MeshStandardMaterial;
      standard.map?.dispose();
      standard.emissiveMap?.dispose();
      material.dispose();
    });
  });
}

async function draw(traits: SkinTraits, mime: "image/webp" | "image/png"): Promise<string> {
  const gltf = await loadModel();
  const view = sharedRenderer();
  const scene = new THREE.Scene();
  scene.background = new THREE.Color("#0a0a0b");
  const root = SkeletonUtils.clone(gltf.scene);
  scene.add(root);
  scene.add(new THREE.HemisphereLight("#8ea0d4", "#2a2018", 0.55));
  scene.add(new THREE.AmbientLight("#4a4036", 0.2));
  const sun = new THREE.DirectionalLight("#ffc49a", 1.1);
  sun.position.set(2.2, 2.4, 2.4);
  scene.add(sun);
  const lamp = new THREE.PointLight("#ffb060", 6, 6, 2);
  lamp.position.set(0.4, 1.2, 1.4);
  scene.add(lamp);
  paint(root, traits);
  const mixer = new THREE.AnimationMixer(root);
  const clip = gltf.animations.find((entry) => entry.name === "idle");
  if (clip) {
    mixer.clipAction(clip).play();
    mixer.update(0.4);
  }
  const face = mountFace(root, sourceName);
  const camera = new THREE.PerspectiveCamera(26, WIDTH / HEIGHT, 0.08, 24);
  fitCamera(camera, root);
  if (face) {
    redrawFace(face, {
      style: traits.eyes,
      expression: "calm",
      blink: 0,
      lookX: Math.max(-1, Math.min(1, camera.position.x * 0.8)),
      lookY: Math.max(-1, Math.min(1, (camera.position.y - 0.7) * 0.6)),
      color: screenColor(traits.screen),
    });
  }
  view.render(scene, camera);
  const url = mime === "image/png"
    ? view.domElement.toDataURL("image/png")
    : view.domElement.toDataURL("image/webp", 0.9);
  mixer.stopAllAction();
  release(root);
  scene.clear();
  return url.startsWith("data:image/") ? url : "";
}

export function renderPortrait(traits: SkinTraits): Promise<string> {
  const key = portraitKey(traits);
  const hit = cache.get(key);
  if (hit) return Promise.resolve(hit);
  const running = inflight.get(key);
  if (running) return running;
  const job = chain.then(async () => {
    try {
      const url = await draw(traits, "image/webp");
      if (url) cache.set(key, url);
      return url;
    } catch {
      return "";
    } finally {
      inflight.delete(key);
    }
  });
  inflight.set(key, job);
  chain = job.then(
    () => undefined,
    () => undefined,
  );
  return job;
}

/** Lossless frame of the same 480×640 still the market card caches. */
export function capturePortraitPng(traits: SkinTraits): Promise<string> {
  return draw(traits, "image/png");
}
