/**
 * Enter shots for the five rooms. Fails if #frame-audit reports an overlap,
 * a board cut off the frame, or an agent under 3 m.
 *
 *   NODE_PATH=/tmp/pw/node_modules node tools/shot-interiors.mjs
 */
import fs from "node:fs";
import { pathToFileURL } from "node:url";

const playwrightPath = "/tmp/pw/node_modules/playwright-core/index.js";
const playwright = await import(pathToFileURL(playwrightPath).href);
const { chromium } = playwright.default;

const outDir = process.argv[2] || "/opt/cursor/artifacts";
const rooms = ["vault", "market", "war", "lobby", "stadium"];
const ids = {
  vault: "vault-quarter",
  market: "trading-village",
  war: "war-room",
  lobby: "skin-forge",
  stadium: "cup-stadium",
};

const browser = await chromium.launch({
  executablePath: "/usr/local/bin/google-chrome",
  args: ["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist", "--no-sandbox"],
});
const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
const errors = [];
page.on("pageerror", (err) => errors.push(String(err)));
page.on("console", (msg) => {
  if (msg.type() === "error") errors.push(msg.text());
});
await page.addInitScript(() => {
  localStorage.setItem(
    "fe2vi.spectator.v1",
    JSON.stringify({ entered: true, tutorial: 4, quests: { heist: true, follow: true, breach: true } }),
  );
});

const audits = {};
let failed = false;
for (const room of rooms) {
  for (const wide of [false, true]) {
    const id = ids[room];
    const url = `http://127.0.0.1:3000/?frame=${id}${wide ? "-out" : ""}`;
    await page.goto(url, { waitUntil: "domcontentloaded", timeout: 30000 });
    await page.waitForTimeout(3200);
    const file = `${outDir}/interior-${room}${wide ? "-wide" : ""}-fix.png`;
    if (!wide) {
      const audit = await page.evaluate(async () => {
        const deadline = performance.now() + 5000;
        while (performance.now() < deadline) {
          const node = document.getElementById("frame-audit");
          if (node && node.textContent) {
            const data = JSON.parse(node.textContent);
            if (data.ready) return data;
          }
          await new Promise((resolve) => setTimeout(resolve, 150));
        }
        const node = document.getElementById("frame-audit");
        return node && node.textContent ? JSON.parse(node.textContent) : { ready: false, ok: false, issues: ["audit missing"] };
      });
      audits[room] = audit;
      if (!audit.ok) {
        failed = true;
        console.error("FAIL", room, JSON.stringify(audit.issues || audit));
      } else {
        console.log("OK", room, audit.agents.map((agent) => `${agent.name} ${agent.meters}m`).join(", "));
      }
    }
    const client = await page.context().newCDPSession(page);
    const image = await client.send("Page.captureScreenshot", { format: "png" });
    fs.writeFileSync(file, Buffer.from(image.data, "base64"));
    const draws = await page.evaluate(() => document.getElementById("draw-readout")?.textContent || "");
    console.log("SHOT", file.split("/").pop(), "draws", draws);
    await client.detach();
  }
}
fs.writeFileSync("/tmp/frame-audit.json", JSON.stringify({ audits, errors: errors.slice(0, 12) }, null, 2));
await browser.close();
if (failed || errors.length) {
  console.error("page errors", errors.slice(0, 6));
  process.exit(1);
}
