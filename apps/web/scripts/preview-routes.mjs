// Required before every push that can reach fe2vi.space.
// Builds the OpenNext worker and fails unless workerd returns 200 for the routes below.
import { spawn } from "node:child_process";
import { setTimeout as delay } from "node:timers/promises";

const port = 8787;
const origin = `http://127.0.0.1:${port}`;
const routes = ["/", "/?room=vault", "/lobby", "/leaderboard"];
const env = {
  ...process.env,
  NEXT_PUBLIC_FE2VI_API_URL: process.env.NEXT_PUBLIC_FE2VI_API_URL || "https://api.fe2vi.space",
};

function run(command, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd: process.cwd(), env, stdio: "inherit" });
    child.on("error", reject);
    child.on("exit", (code) => {
      if (code === 0) resolve();
      else reject(new Error(`${command} ${args.join(" ")} exited ${code}`));
    });
  });
}

await run("npx", ["opennextjs-cloudflare", "build"]);

const preview = spawn("npx", ["opennextjs-cloudflare", "preview", "--", "--port", String(port), "--ip", "127.0.0.1"], {
  cwd: process.cwd(),
  env,
  detached: true,
  stdio: ["ignore", "pipe", "pipe"],
});
let log = "";
const take = (chunk) => {
  const text = chunk.toString();
  log += text;
  process.stdout.write(text);
};
preview.stdout?.on("data", take);
preview.stderr?.on("data", take);

function stop() {
  if (!preview.pid) return;
  try {
    process.kill(-preview.pid, "SIGTERM");
  } catch {
    preview.kill("SIGTERM");
  }
}

let up = false;
for (let attempt = 0; attempt < 90; attempt += 1) {
  if (preview.exitCode !== null) break;
  try {
    const probe = await fetch(origin + "/");
    if (probe.status > 0) {
      up = true;
      break;
    }
  } catch {
    // wrangler is still booting
  }
  await delay(1000);
}

if (!up) {
  console.error(log.slice(-2000));
  stop();
  process.exit(1);
}

let failed = false;
for (const route of routes) {
  const response = await fetch(origin + route, { redirect: "manual" });
  const body = await response.text();
  const broken = body.includes("readFileSync is not implemented") || body.includes("Internal Server Error");
  console.log(`${response.status} ${route}${broken ? " BODY_ERROR" : ""}`);
  if (response.status !== 200 || broken) failed = true;
}

stop();
await delay(400);
process.exit(failed ? 1 : 0);
