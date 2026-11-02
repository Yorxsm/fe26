// Fails when a direct room link renders an empty plot, or when a building click
// updates the camera probe but the canvas keeps showing the overview.
// Headless Chrome + SwiftShader. Compares pixels, not just window.__fe2viRoom.
import { spawn } from "node:child_process";
import { setTimeout as delay } from "node:timers/promises";
import sharp from "sharp";

const origin = (process.argv[2] ?? "http://127.0.0.1:3210").replace(/\/$/, "");
const port = 9355;
const chrome = process.env.CHROME_PATH || "google-chrome";

const browser = spawn(
  chrome,
  [
    "--headless=new",
    "--use-gl=angle",
    "--use-angle=swiftshader",
    "--enable-unsafe-swiftshader",
    `--remote-debugging-port=${port}`,
    "--user-data-dir=/tmp/fe2vi-room-check",
    "--no-first-run",
    "--disable-gpu-sandbox",
    "--no-sandbox",
    "about:blank",
  ],
  { stdio: "ignore" },
);

function stop() {
  browser.kill("SIGTERM");
}

let up = false;
for (let attempt = 0; attempt < 40; attempt += 1) {
  try {
    const probe = await fetch(`http://127.0.0.1:${port}/json/version`);
    if (probe.ok) {
      up = true;
      break;
    }
  } catch {
    // chrome is still booting
  }
  await delay(250);
}
if (!up) {
  stop();
  console.error("Chrome did not open a debugging port");
  process.exit(1);
}

const created = await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: "PUT" });
const tab = await created.json();
const ws = new WebSocket(tab.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  ws.addEventListener("open", resolve);
  ws.addEventListener("error", reject);
});
let seq = 0;
const waiters = new Map();
ws.addEventListener("message", (event) => {
  const msg = JSON.parse(event.data);
  if (msg.id && waiters.has(msg.id)) {
    waiters.get(msg.id)(msg);
    waiters.delete(msg.id);
  }
});
function send(method, params = {}) {
  const id = ++seq;
  ws.send(JSON.stringify({ id, method, params }));
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`timeout ${method}`)), 45000);
    waiters.set(id, (msg) => {
      clearTimeout(timer);
      if (msg.error) reject(new Error(JSON.stringify(msg.error)));
      else resolve(msg.result ?? {});
    });
  });
}

await send("Page.enable");
await send("Runtime.enable");
await send("Page.addScriptToEvaluateOnNewDocument", {
  source: `window.__errs=[];addEventListener("error",(e)=>window.__errs.push(String(e.message)));addEventListener("unhandledrejection",(e)=>window.__errs.push(String(e.reason)));const orig=console.error;console.error=(...a)=>{window.__errs.push(a.map(String).join(" ").slice(0,400));orig.apply(console,a);};`,
});
await send("Page.bringToFront");
await send("Emulation.setDeviceMetricsOverride", { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
async function openOverview() {
  const clickText = async (pattern) =>
    send("Runtime.evaluate", {
      expression: `[...document.querySelectorAll('button')].find((button) => ${pattern}.test(button.textContent))?.click()`,
    });
  for (let attempt = 0; attempt < 40; attempt += 1) {
    await delay(500);
    const ready = await send("Runtime.evaluate", {
      expression: `document.querySelector('nav.room-focus') ? 'city' : [...document.querySelectorAll('button')].some((button) => /Enter as spectator/i.test(button.textContent)) ? 'title' : ''`,
      returnByValue: true,
    });
    const screen = ready.result?.value;
    if (screen === "city") return true;
    if (screen === "title") {
      await clickText("/Enter as spectator/i");
      await delay(300);
      await clickText("/Skip/i");
      await delay(300);
      await clickText("/Stay on the overview/i");
    }
  }
  return false;
}

async function layoutReport() {
  const result = await send("Runtime.evaluate", {
    expression: `(() => {
      const root = document.documentElement;
      const box = (node) => {
        if (!node) return null;
        const rect = node.getBoundingClientRect();
        if (rect.width < 2 || rect.height < 2) return null;
        return { x: rect.x, y: rect.y, w: rect.width, h: rect.height };
      };
      const targets = [...document.querySelectorAll("button, a.btn")].map((node) => {
        const rect = box(node);
        const label = (node.getAttribute("title") || node.getAttribute("aria-label") || node.textContent || "").replace(/\\s+/g, " ").trim();
        return rect ? { label: label.slice(0, 32), ...rect } : null;
      }).filter(Boolean);
      const panels = [".hud-bl", ".hud-bc", ".room-hud", ".quest-hint", ".hud-aside"].map((selector) => ({ selector, ...box(document.querySelector(selector)) || {} })).filter((panel) => panel.w);
      const overlaps = [];
      for (let i = 0; i < panels.length; i += 1) {
        for (let j = i + 1; j < panels.length; j += 1) {
          const a = panels[i];
          const b = panels[j];
          const x = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x);
          const y = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);
          if (x > 8 && y > 8) overlaps.push([a.selector, b.selector, Math.round(x), Math.round(y)]);
        }
      }
      return {
        scroll: root.scrollWidth - root.clientWidth,
        targets,
        panels,
        overlaps,
        offline: /could not open the city/i.test(document.body.innerText),
      };
    })()`,
    returnByValue: true,
  });
  return result.result?.value ?? null;
}

if (process.env.LAYOUT_ONLY) {
  const sizes = [[390, 844], [360, 740]];
  const problems = [];
  for (const [width, height] of sizes) {
    await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: true });
    await send("Page.navigate", { url: `${origin}/` });
    const city = await openOverview();
    for (let attempt = 0; attempt < 30; attempt += 1) {
      const ready = await send("Runtime.evaluate", {
        expression: `!document.querySelector('.loading-screen') && document.querySelector('canvas') ? 'ready' : ''`,
        returnByValue: true,
      });
      if (ready.result?.value === "ready") break;
      await delay(400);
    }
    await delay(800);
    const overview = await layoutReport();
    const shot = await send("Page.captureScreenshot", { format: "png" });
    await import("node:fs/promises").then((fs) => fs.writeFile(`/tmp/fe2vi-m-${width}.png`, Buffer.from(shot.data, "base64")));
    const want = ["Home", "Games", "Hints"];
    for (const label of want) {
      const hit = (overview?.targets || []).find((item) => item.label === label || item.label.startsWith(label));
      if (!hit || hit.h < 44 || hit.w < 44) problems.push(`${width} missing ${label} ${JSON.stringify(hit)}`);
    }
    if ((overview?.scroll ?? 1) > 1) problems.push(`${width} horizontal scroll ${overview.scroll}`);
    if (overview?.overlaps?.length) problems.push(`${width} overlap ${JSON.stringify(overview.overlaps)}`);
    if (!city || overview?.offline) problems.push(`${width} city did not open`);
    await send("Runtime.evaluate", {
      expression: `[...document.querySelectorAll('button')].find((button) => /Season Hall/i.test(button.textContent))?.click()`,
    });
    await delay(1200);
    const room = await layoutReport();
    const roomShot = await send("Page.captureScreenshot", { format: "png" });
    await import("node:fs/promises").then((fs) => fs.writeFile(`/tmp/fe2vi-m-room-${width}.png`, Buffer.from(roomShot.data, "base64")));
    for (const label of ["Leave room", "Home", "Games"]) {
      const hit = (room?.targets || []).find((item) => item.label === label || item.label.startsWith(label));
      if (!hit || hit.h < 44 || hit.w < 44) problems.push(`${width} room missing ${label} ${JSON.stringify(hit)}`);
    }
    if ((room?.scroll ?? 1) > 1) problems.push(`${width} room scroll ${room.scroll}`);
    if (room?.overlaps?.length) problems.push(`${width} room overlap ${JSON.stringify(room.overlaps)}`);
    await send("Runtime.evaluate", {
      expression: `[...document.querySelectorAll('button')].find((button) => /Room panel/i.test(button.textContent))?.click()`,
    });
    await delay(400);
    const openShot = await send("Page.captureScreenshot", { format: "png" });
    await import("node:fs/promises").then((fs) => fs.writeFile(`/tmp/fe2vi-m-open-${width}.png`, Buffer.from(openShot.data, "base64")));
    const open = await layoutReport();
    if ((open?.scroll ?? 1) > 1) problems.push(`${width} sheet scroll ${open.scroll}`);
    if (open?.overlaps?.length) problems.push(`${width} sheet overlap ${JSON.stringify(open.overlaps)}`);
    console.log(JSON.stringify({ width, overview: { scroll: overview?.scroll, overlaps: overview?.overlaps, targets: overview?.targets }, room: { scroll: room?.scroll, overlaps: room?.overlaps, targets: room?.targets } }));
  }
  ws.close();
  stop();
  if (problems.length) {
    console.error(problems.join("\n"));
    process.exit(1);
  }
  console.log("layout ok");
  process.exit(0);
}
if (process.env.HOME_ONLY) {
  await send("Page.navigate", { url: `${origin}/` });
  await delay(8000);
  const home = await send("Runtime.evaluate", {
    expression: `({ canvases: document.querySelectorAll('canvas').length, text: (document.body?.innerText || '').slice(0, 160), errs: (window.__errs || []).slice(0, 4) })`,
    returnByValue: true,
  });
  console.log("home", JSON.stringify(home.result?.value));
  ws.close();
  stop();
  process.exit(0);
}
await send("Page.navigate", { url: `${origin}/?room=lobby` });

const expr = `window.__fe2viRoom ?? null`;
let sample = null;
for (let attempt = 0; attempt < 40; attempt += 1) {
  await delay(500);
  const result = await send("Runtime.evaluate", { expression: expr, returnByValue: true });
  sample = result.result?.value ?? null;
  const meshes = Number(sample?.meshes ?? 0);
  const camera = sample?.camera;
  if (meshes > 0 && camera) break;
}
const direct = sample;
await send("Page.navigate", { url: `${origin}/` });
const clickText = async (pattern) =>
  send("Runtime.evaluate", {
    expression: `[...document.querySelectorAll('button')].find((button) => ${pattern}.test(button.textContent))?.click()`,
  });
let overview = false;
for (let attempt = 0; attempt < 40; attempt += 1) {
  await delay(500);
  const ready = await send("Runtime.evaluate", {
    expression: `document.querySelector('nav.room-focus') ? 'city' : [...document.querySelectorAll('button')].some((button) => /Enter as spectator/i.test(button.textContent)) ? 'title' : ''`,
    returnByValue: true,
  });
  const screen = ready.result?.value;
  if (screen === "city") {
    overview = true;
    break;
  }
  if (screen === "title") {
    await clickText("/Enter as spectator/i");
    await delay(300);
    await clickText("/Skip/i");
    await delay(300);
    await clickText("/Stay on the overview/i");
  }
}
if (!overview) {
  stop();
  console.error("The overview never opened");
  process.exit(1);
}
let doorHits = 0;
for (let attempt = 0; attempt < 20; attempt += 1) {
  await delay(500);
  const covered = await send("Runtime.evaluate", {
    expression: `(() => {
      const doors = window.__fe2viDoors || [];
      const canvas = document.querySelector("canvas");
      if (!canvas || document.querySelector(".loading-screen")) return { hit: 0 };
      const rect = canvas.getBoundingClientRect();
      let hit = 0;
      for (const door of doors) {
        if (door.x < rect.left + 8 || door.x > rect.right - 8 || door.y < rect.top + 8 || door.y > rect.bottom - 64) continue;
        if (document.elementFromPoint(door.x, door.y) === canvas) hit += 1;
      }
      return { hit };
    })()`,
    returnByValue: true,
  });
  doorHits = covered.result?.value?.hit ?? 0;
  if (doorHits >= 6) break;
}
const viewOf = async () => {
  const result = await send("Runtime.evaluate", {
    expression: `(() => {
      const canvas = document.querySelector("canvas");
      if (!canvas) return null;
      const rect = canvas.getBoundingClientRect();
      return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
    })()`,
    returnByValue: true,
  });
  return result.result?.value ?? null;
};
const grab = async (name) => {
  const view = await viewOf();
  if (!view || view.width < 32 || view.height < 32) return null;
  const shot = await send("Page.captureScreenshot", { format: "png", fromSurface: true });
  const png = Buffer.from(shot.data, "base64");
  if (name) await import("node:fs/promises").then((fs) => fs.writeFile(`/tmp/fe2vi-${name}.png`, png));
  const left = Math.round(view.x + view.width * 0.42);
  const top = Math.round(view.y + view.height * 0.18);
  const width = Math.round(view.width * 0.4);
  const height = Math.round(view.height * 0.48);
  const { data, info } = await sharp(png)
    .extract({ left, top, width, height })
    .resize(96, 64, { fit: "fill" })
    .removeAlpha()
    .raw()
    .toBuffer({ resolveWithObject: true });
  return { data, info, view };
};
const meanDelta = (a, b) => {
  const count = Math.min(a.length, b.length);
  let sum = 0;
  for (let i = 0; i < count; i += 1) sum += Math.abs(a[i] - b[i]);
  return count ? sum / count : 0;
};
const before = await grab("before");
let clicked = null;
for (let attempt = 0; attempt < 24; attempt += 1) {
  const mark = await send("Runtime.evaluate", {
    expression: `(() => {
      const doors = window.__fe2viDoors || [];
      const canvas = document.querySelector('canvas');
      if (!canvas) return null;
      const rect = canvas.getBoundingClientRect();
      const order = ['oracle-plaza', 'war-room', 'cup-stadium', 'vault-quarter', 'trading-village', 'skin-forge'];
      for (const id of order) {
        const door = doors.find((entry) => entry.id === id);
        if (!door) continue;
        if (door.x < rect.left + 12 || door.x > rect.right - 12 || door.y < rect.top + 12 || door.y > rect.bottom - 72) continue;
        const el = document.elementFromPoint(door.x, door.y);
        if (el === canvas) return { id, x: door.x, y: door.y };
      }
      return null;
    })()`,
    returnByValue: true,
  });
  const hall = mark.result?.value;
  if (!hall || !Number.isFinite(hall.x) || !Number.isFinite(hall.y)) {
    await delay(300);
    continue;
  }
  await send("Input.dispatchMouseEvent", { type: "mouseMoved", x: hall.x, y: hall.y });
  await send("Input.dispatchMouseEvent", { type: "mousePressed", x: hall.x, y: hall.y, button: "left", clickCount: 1 });
  await send("Input.dispatchMouseEvent", { type: "mouseReleased", x: hall.x, y: hall.y, button: "left", clickCount: 1 });
  for (let wait = 0; wait < 10; wait += 1) {
    await delay(400);
    const result = await send("Runtime.evaluate", { expression: expr, returnByValue: true });
    clicked = result.result?.value ?? null;
    const meshes = Number(clicked?.meshes ?? 0);
    const camera = clicked?.camera;
    if (meshes > 0 && camera && camera.y > 0.8 && camera.y < 6 && clicked?.focusId) break;
  }
  if (Number(clicked?.meshes ?? 0) > 0 && clicked?.focusId) break;
}
await delay(3000);
const after = await grab("after");
const slugById = {
  "oracle-plaza": "hall",
  "war-room": "war",
  "cup-stadium": "cup",
  "vault-quarter": "vault",
  "trading-village": "village",
  "skin-forge": "lobby",
};
const clickedId = clicked?.focusId;
const roomSlug = slugById[clickedId] ?? "hall";
await send("Page.navigate", { url: `${origin}/?room=${roomSlug}` });
let directRoom = null;
for (let attempt = 0; attempt < 40; attempt += 1) {
  await delay(500);
  const result = await send("Runtime.evaluate", {
    expression: `({ room: window.__fe2viRoom ?? null, loading: Boolean(document.querySelector(".loading-screen")) })`,
    returnByValue: true,
  });
  const value = result.result?.value ?? {};
  directRoom = value.room ?? null;
  if (Number(directRoom?.meshes ?? 0) > 0 && directRoom?.focusId === clickedId && !value.loading) break;
}
await delay(800);
const linked = await grab("linked");
const changed = before && after ? meanDelta(before.data, after.data) : 0;
const versusLink = after && linked ? meanDelta(after.data, linked.data) : 999;
ws.close();
stop();

const meshes = Number(direct?.meshes ?? 0);
const camera = direct?.camera;
const inside =
  camera &&
  camera.y > 0.8 &&
  camera.y < 6 &&
  Math.hypot(camera.x + 38, camera.z + 24) < 14;
const clickMeshes = Number(clicked?.meshes ?? 0);
const clickCamera = clicked?.camera;
const clickInside = clickCamera && clickCamera.y > 0.8 && clickCamera.y < 6 && clickMeshes > 0;
console.log(JSON.stringify({
  direct: { meshes, camera, focusId: direct?.focusId ?? null },
  click: { meshes: clickMeshes, camera: clickCamera, focusId: clicked?.focusId ?? null },
  pixels: { changed: Number(changed.toFixed(2)), versusLink: Number(versusLink.toFixed(2)) },
}));
if (meshes < 1 || !inside || !clickInside) {
  console.error("Room rendered empty or the camera stayed outside");
  process.exit(1);
}
// The center of the canvas has to leave the overview. A room panel opening
// is not enough: that chrome sits outside this crop. The click frame also
// has to resemble the same room opened with ?room=, which does render.
if (!before || !after || changed < 12) {
  console.error("Click left the overview on screen");
  process.exit(1);
}
if (linked && versusLink > 60) {
  console.error("Click frame does not match the room link");
  process.exit(1);
}
if (doorHits < 6) {
  console.error(`Only ${doorHits} buildings are clickable from the overview`);
  process.exit(1);
}
console.log("room interior ok");
