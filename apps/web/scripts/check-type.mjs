import fs from "node:fs";
import path from "node:path";

const allowed = new Set([
  "--type-display",
  "--type-h1",
  "--type-h2",
  "--type-h3",
  "--type-body",
  "--type-small",
  "--type-caption",
]);
const pxOk = new Set(["12px", "14px", "16px", "20px", "24px", "32px", "48px"]);

function walk(dir, out = []) {
  for (const name of fs.readdirSync(dir)) {
    if (name === "node_modules" || name === ".next") continue;
    const full = path.join(dir, name);
    if (fs.statSync(full).isDirectory()) walk(full, out);
    else if (/\.(css|tsx|ts)$/.test(name)) out.push(full);
  }
  return out;
}

const errors = [];
for (const file of walk(".")) {
  const text = fs.readFileSync(file, "utf8");
  if (/Unbounded|Manrope/.test(text)) errors.push(`${file}: Unbounded or Manrope remains`);
  if (!file.endsWith(".css")) continue;
  if (/gradient/.test(text)) errors.push(`${file}: gradient remains`);
  const fonts = /font-size:\s*([^;]+);/g;
  let match;
  while ((match = fonts.exec(text))) {
    const value = match[1].trim();
    const token = value.match(/^var\((--type-[a-z0-9-]+)\)$/);
    if (token && allowed.has(token[1])) continue;
    errors.push(`${file}: raw font-size ${value}`);
  }
  const defs = /--type-[a-z0-9-]+:\s*([^;]+);/g;
  while ((match = defs.exec(text))) {
    if (!pxOk.has(match[1].trim())) errors.push(`${file}: type token ${match[1].trim()} is outside the scale`);
  }
}

if (errors.length) {
  console.error(errors.join("\n"));
  process.exit(1);
}
console.log("type scale ok");
