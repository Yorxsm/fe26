/**
 * Rebuild the village models from apps/web/models-src.
 *
 *   npm run optimize-models
 *
 * station.glb is split. The core keeps terrain, roads, and pads. Trees, the
 * duplicate building shells, and the removed pier, club, and oracle scaffolds
 * are pruned. kits.glb keeps the building kits. Both get dedupe, prune, weld,
 * WebP textures (1024 desktop, 512 mobile) and EXT_meshopt_compression.
 * MeshoptDecoder ships inside the Three.js bundle, so no external decoder URL is used.
 * Kit textures stay as authored, because recompressing the atlas drops the alpha mask.
 */
import { mkdir, copyFile, stat, unlink } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { NodeIO } from "@gltf-transform/core";
import { ALL_EXTENSIONS, EXTMeshoptCompression } from "@gltf-transform/extensions";
import { cloneDocument, dedup, meshopt, prune, textureCompress, weld } from "@gltf-transform/functions";
import { MeshoptDecoder, MeshoptEncoder } from "meshoptimizer";
import sharp from "sharp";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const srcDir = path.join(root, "models-src");
const outDir = path.join(root, "public", "models");

const KEEP_PREFIX = new Set(["terrain", "road", "marker"]);

function keepStationNode(name) {
  if (!name) return true;
  const prefix = name.split(".")[0];
  if (KEEP_PREFIX.has(prefix)) return true;
  return name.endsWith(".pad");
}

async function ensureSource(name) {
  const destination = path.join(srcDir, name);
  try {
    await stat(destination);
    return destination;
  } catch {
    await mkdir(srcDir, { recursive: true });
    const from = path.join(outDir, name);
    await copyFile(from, destination);
    return destination;
  }
}

function dropNodes(document, keep) {
  for (const node of document.getRoot().listNodes()) {
    if (!keep(node.getName() || "")) node.dispose();
  }
}

async function writeVariant(document, file, resize, geometry) {
  const copy = cloneDocument(document);
  if (geometry === "fresh") copy.createExtension(EXTMeshoptCompression).dispose();
  const steps = [dedup(), prune()];
  if (geometry === "meshopt") {
    steps.push(weld({ tolerance: 0.0001 }));
    steps.push(
      textureCompress({
        encoder: sharp,
        targetFormat: "webp",
        resize,
        quality: 72,
        effort: 6,
      }),
    );
  }
  steps.push(meshopt({ encoder: MeshoptEncoder, level: "high" }));
  await copy.transform(...steps);
  const target = path.join(outDir, file);
  await io.write(target, copy);
  const info = await stat(target);
  console.log(`${file} ${Math.round(info.size / 1024)} KB`);
}

await MeshoptEncoder.ready;
await MeshoptDecoder.ready;

const io = new NodeIO().registerExtensions(ALL_EXTENSIONS).registerDependencies({
  "meshopt.encoder": MeshoptEncoder,
  "meshopt.decoder": MeshoptDecoder,
});

const stationPath = await ensureSource("station.glb");
const kitsPath = await ensureSource("kits.glb");

const station = await io.read(stationPath);
dropNodes(station, keepStationNode);
await writeVariant(station, "station-core.glb", [1024, 1024], "meshopt");
await writeVariant(station, "station-core-512.glb", [512, 512], "meshopt");
await unlink(path.join(outDir, "station.glb")).catch(() => undefined);

const kits = await io.read(kitsPath);
await writeVariant(kits, "kits.glb", [1024, 1024], "fresh");
await unlink(path.join(outDir, "kits-512.glb")).catch(() => undefined);
