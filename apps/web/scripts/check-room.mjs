// Fails when a direct room link renders an empty plot.
// Headless Chrome + SwiftShader. Asserts the interior mesh count and that the
// camera is inside the Lobby shell.
import { spawn } from "node:child_process";
import { setTimeout as delay } from "node:timers/promises";

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
    const timer = setTimeout(() => reject(new Error(`timeout ${method}`)), 20000);
    waiters.set(id, (msg) => {
      clearTimeout(timer);
      if (msg.error) reject(new Error(JSON.stringify(msg.error)));
      else resolve(msg.result ?? {});
    });
  });
}

await send("Page.enable");
await send("Runtime.enable");
await send("Page.bringToFront");
await send("Emulation.setDeviceMetricsOverride", { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
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
await delay(1200);
let clicked = null;
for (let attempt = 0; attempt < 24; attempt += 1) {
  const mark = await send("Runtime.evaluate", {
    expression: `(() => {
      const doors = window.__fe2viDoors || [];
      const canvas = document.querySelector('canvas');
      const order = ['oracle-plaza', 'war-room', 'cup-stadium', 'vault-quarter', 'trading-village', 'skin-forge'];
      for (const id of order) {
        const door = doors.find((entry) => entry.id === id);
        if (!door) continue;
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
console.log(JSON.stringify({ direct: { meshes, camera, focusId: direct?.focusId ?? null }, click: { meshes: clickMeshes, camera: clickCamera, focusId: clicked?.focusId ?? null } }));
if (meshes < 1 || !inside || !clickInside) {
  console.error("Room rendered empty or the camera stayed outside");
  process.exit(1);
}
console.log("room interior ok");
