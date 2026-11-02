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
await delay(1500);
await send("Runtime.evaluate", {
  expression: `[...document.querySelectorAll('button')].find((button) => /Enter as spectator/i.test(button.textContent))?.click()`,
});
await delay(400);
await send("Runtime.evaluate", {
  expression: `[...document.querySelectorAll('button')].find((button) => /Skip/i.test(button.textContent))?.click()`,
});
await delay(400);
await send("Runtime.evaluate", {
  expression: `[...document.querySelectorAll('button')].find((button) => /Stay on the overview/i.test(button.textContent))?.click()`,
});
await delay(3500);
const box = await send("Runtime.evaluate", {
  expression: `(() => { const canvas = document.querySelector('canvas'); const rect = canvas.getBoundingClientRect(); return { x: rect.x, y: rect.y, w: rect.width, h: rect.height }; })()`,
  returnByValue: true,
});
const canvas = box.result?.value ?? { x: 0, y: 0, w: 1440, h: 900 };
const spots = [
  [0.7, 0.42],
  [0.62, 0.46],
  [0.5, 0.5],
  [0.35, 0.48],
];
let clicked = null;
for (const [px, py] of spots) {
  const x = canvas.x + canvas.w * px;
  const y = canvas.y + canvas.h * py;
  await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
  await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", clickCount: 1 });
  await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", clickCount: 1 });
  for (let attempt = 0; attempt < 8; attempt += 1) {
    await delay(400);
    const result = await send("Runtime.evaluate", { expression: expr, returnByValue: true });
    clicked = result.result?.value ?? null;
    const meshes = Number(clicked?.meshes ?? 0);
    const camera = clicked?.camera;
    if (meshes > 0 && camera && camera.y > 0.8 && camera.y < 6) break;
  }
  if (Number(clicked?.meshes ?? 0) > 0 && clicked?.camera && clicked.camera.y > 0.8 && clicked.camera.y < 6) break;
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
