#!/usr/bin/env node
// Render the market-card portrait still for gen_001..gen_050 and disp_001.
// The browser path is the same renderer the site uses (480×640). Each frame
// is then stored as a 300×400 WebP in assets/passport-stills.
import { spawn } from "node:child_process";
import { createRequire } from "node:module";
import { createServer } from "node:http";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const sharp = require("../apps/web/node_modules/sharp");
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const web = path.join(root, "apps/web");
const outDir = path.join(root, "assets/passport-stills");
const bundlePath = "/tmp/fe2vi-portrait-bundle.js";
const defaultIds = [...Array(50)].map((_, index) => `gen_${String(index + 1).padStart(3, "0")}`).concat(["disp_001"]);
const ids = process.argv.slice(2).length ? process.argv.slice(2) : defaultIds;

function fnv(tokenId) {
  let acc = 2166136261;
  for (let i = 0; i < tokenId.length; i += 1) {
    acc ^= tokenId.charCodeAt(i);
    acc = Math.imul(acc, 16777619) >>> 0;
  }
  return { paint: acc % 12, headwear: (acc >>> 10) % 8, screen: (acc >>> 4) % 8 };
}

function run(command, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { stdio: "inherit" });
    child.on("exit", (code) => (code === 0 ? resolve() : reject(new Error(`${command} exited ${code}`))));
  });
}

function serve(publicDir, bundle) {
  const html = `<!doctype html><meta charset="utf-8"><script src="/bundle.js"></script>`;
  return new Promise((resolve) => {
    const server = createServer((req, res) => {
      const name = decodeURIComponent(new URL(req.url, "http://127.0.0.1").pathname);
      if (name === "/capture.html") {
        res.writeHead(200, { "content-type": "text/html" });
        res.end(html);
        return;
      }
      if (name === "/bundle.js") {
        res.writeHead(200, { "content-type": "text/javascript" });
        res.end(bundle);
        return;
      }
      const file = path.join(publicDir, path.normalize(name));
      if (!file.startsWith(publicDir)) {
        res.writeHead(403);
        res.end();
        return;
      }
      try {
        const body = readFileSync(file);
        res.writeHead(200);
        res.end(body);
      } catch {
        res.writeHead(404);
        res.end();
      }
    });
    server.listen(0, "127.0.0.1", () => resolve(server));
  });
}

function cdp(ws) {
  let next = 0;
  const waiters = new Map();
  const events = [];
  ws.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    if (message.id && waiters.has(message.id)) {
      waiters.get(message.id)(message);
      waiters.delete(message.id);
      return;
    }
    events.push(message);
  });
  return {
    send(method, params = {}) {
      const id = ++next;
      return new Promise((resolve, reject) => {
        waiters.set(id, (message) => (message.error ? reject(new Error(JSON.stringify(message.error))) : resolve(message.result)));
        ws.send(JSON.stringify({ id, method, params }));
      });
    },
    wait(method, timeoutMs) {
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error(`timed out waiting for ${method}`)), timeoutMs);
        const poll = () => {
          const index = events.findIndex((event) => event.method === method);
          if (index >= 0) {
            clearTimeout(timer);
            resolve(events.splice(index, 1)[0]);
            return;
          }
          setTimeout(poll, 30);
        };
        poll();
      });
    },
  };
}

async function encodeStill(png) {
  const widths = [300, 320, 288, 256];
  let closest = null;
  for (const width of widths) {
    const height = Math.round(width * 4 / 3);
    for (let quality = 100; quality >= 35; quality -= 5) {
      const image = await sharp(png).resize(width, height, { fit: "fill" }).webp({ quality }).toBuffer();
      const distance = image.length < 8192 ? 8192 - image.length : image.length > 15360 ? image.length - 15360 : 0;
      if (!closest || distance < closest.distance) closest = { image, distance, width, quality };
      if (distance === 0) return closest;
    }
  }
  throw new Error(`could not fit a still in 8-15 KB (closest ${closest?.image.length ?? 0} bytes at ${closest?.width}px q${closest?.quality})`);
}

async function main() {
  await run(path.join(web, "node_modules/esbuild/bin/esbuild"), [
    path.join(web, "scripts/portrait-capture-entry.ts"),
    "--bundle",
    "--format=iife",
    "--platform=browser",
    `--alias:@=${web}`,
    `--outfile=${bundlePath}`,
  ]);
  const bundle = readFileSync(bundlePath);
  const server = await serve(path.join(web, "public"), bundle);
  const port = server.address().port;
  const chrome = spawn("google-chrome", [
    "--headless=new",
    "--disable-gpu",
    "--no-sandbox",
    "--enable-unsafe-swiftshader",
    `--user-data-dir=/tmp/fe2vi-chrome-stills-${process.pid}`,
    "--remote-debugging-port=9355",
    `http://127.0.0.1:${port}/capture.html`,
  ], { stdio: "ignore" });
  try {
    let page;
    for (let attempt = 0; attempt < 40; attempt += 1) {
      try {
        const listed = await fetch("http://127.0.0.1:9355/json/list").then((response) => response.json());
        page = listed.find((entry) => entry.url.includes("capture.html")) ?? listed[0];
        if (page?.webSocketDebuggerUrl) break;
      } catch {
        page = null;
      }
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
    if (!page?.webSocketDebuggerUrl) throw new Error("Chrome did not open the capture page");
    const ws = new WebSocket(page.webSocketDebuggerUrl);
    await new Promise((resolve, reject) => {
      ws.addEventListener("open", resolve);
      ws.addEventListener("error", reject);
    });
    const client = cdp(ws);
    await client.send("Runtime.enable");
    let ready = false;
    for (let attempt = 0; attempt < 40; attempt += 1) {
      const probed = await client.send("Runtime.evaluate", {
        expression: "typeof window.capturePortrait",
        returnByValue: true,
      });
      if (probed.result?.value === "function") {
        ready = true;
        break;
      }
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
    if (!ready) throw new Error("portrait capture did not load");
    mkdirSync(outDir, { recursive: true });
    const rows = [];
    const frames = new Map();
    for (const tokenId of ids) {
      const evaluated = await client.send("Runtime.evaluate", {
        expression: `window.capturePortrait(${JSON.stringify(tokenId)})`,
        awaitPromise: true,
        returnByValue: true,
      });
      const url = evaluated.result?.value;
      if (typeof url !== "string" || !url.startsWith("data:image/png;base64,")) {
        throw new Error(`no portrait for ${tokenId}: ${JSON.stringify(evaluated).slice(0, 400)}`);
      }
      const png = Buffer.from(url.slice("data:image/png;base64,".length), "base64");
      const still = await encodeStill(png);
      const file = path.join(outDir, `${tokenId}.webp`);
      writeFileSync(file, still.image);
      frames.set(tokenId, { png, still: still.image, width: still.width, quality: still.quality });
      const media = `data:image/webp;base64,${still.image.toString("base64")}`;
      const record = 1 + 4 + tokenId.length + 4 + media.length + 40;
      rows.push({
        tokenId,
        fileBytes: still.image.length,
        width: still.width,
        quality: still.quality,
        mediaBytes: media.length,
        storageBytes: record,
        near: (record / 100000).toFixed(5),
      });
      console.log(`${tokenId} ${still.image.length} bytes ${still.width}px q${still.quality} storage ${record}`);
    }
    ws.close();
    const distinct = [];
    const seen = new Set();
    for (const tokenId of ids) {
      const skin = fnv(tokenId);
      const key = `${skin.paint}:${skin.headwear}:${skin.screen}`;
      if (seen.has(key)) continue;
      seen.add(key);
      distinct.push(tokenId);
      if (distinct.length === 3) break;
    }
    const pairWidth = 480 + 16 + 320;
    const pairHeight = 28 + 640;
    const composites = [];
    for (const tokenId of distinct) {
      const frame = frames.get(tokenId);
      const live = await sharp(frame.png).resize(480, 640, { fit: "fill" }).png().toBuffer();
      const chain = await sharp(frame.still).resize(320, 427, { fit: "fill" }).png().toBuffer();
      const label = await sharp(Buffer.from(
        `<svg width="${pairWidth}" height="28"><rect width="100%" height="100%" fill="#0a0a0b"/><text x="8" y="20" fill="#e7e4dc" font-family="sans-serif" font-size="16">${tokenId} live 480</text><text x="504" y="20" fill="#e7e4dc" font-family="sans-serif" font-size="16">on-chain still</text></svg>`,
      )).png().toBuffer();
      composites.push(await sharp({
        create: { width: pairWidth, height: pairHeight, channels: 3, background: "#0a0a0b" },
      }).composite([
        { input: label, top: 0, left: 0 },
        { input: live, top: 28, left: 0 },
        { input: chain, top: 28, left: 496 },
      ]).png().toBuffer());
    }
    const side = await sharp({
      create: { width: pairWidth, height: pairHeight * composites.length, channels: 3, background: "#0a0a0b" },
    }).composite(composites.map((input, index) => ({ input, top: index * pairHeight, left: 0 }))).png().toBuffer();
    writeFileSync("/tmp/passport-side-by-side.png", side);
    const manifestPath = path.join(outDir, "manifest.json");
    let tokens = rows;
    if (ids.length !== defaultIds.length) {
      let prior = [];
      try {
        prior = JSON.parse(readFileSync(manifestPath, "utf8")).tokens ?? [];
      } catch {
        prior = [];
      }
      const byId = new Map(prior.map((row) => [row.tokenId, row]));
      for (const row of rows) byId.set(row.tokenId, row);
      tokens = [...byId.values()];
    }
    const totalBytes = tokens.reduce((sum, row) => sum + row.storageBytes, 0);
    const manifest = {
      widthNote: "WebP width is chosen per token so the file lands near 8-15 KB.",
      storageByteCostYocto: "10000000000000000000",
      totalStorageBytes: totalBytes,
      totalNear: (totalBytes / 100000).toFixed(5),
      sideBySide: distinct,
      tokens,
    };
    writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
    console.log(`storage bytes ${totalBytes} NEAR ${(totalBytes / 100000).toFixed(5)}`);
    console.log(`side by side /tmp/passport-side-by-side.png (${distinct.join(", ")})`);
  } finally {
    chrome.kill("SIGKILL");
    server.close();
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
