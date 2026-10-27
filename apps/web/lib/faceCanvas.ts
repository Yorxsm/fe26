import * as THREE from "three";
import type { Object3D } from "three";

export type FaceExpression = "calm" | "focused" | "happy" | "shocked";

export type FaceRig = {
  plate: THREE.Mesh;
  color: HTMLCanvasElement;
  glow: HTMLCanvasElement;
  colorTex: THREE.CanvasTexture;
  glowTex: THREE.CanvasTexture;
  time: { value: number };
};

const SIZE = 512;

function sourceOf(node: Object3D, read: (node: Object3D) => string) {
  return read(node);
}

export function expressionForClip(clip: string): FaceExpression {
  if (clip === "celebrate" || clip === "happy") return "happy";
  if (clip === "caught" || clip === "alarm") return "shocked";
  if (clip === "walk" || clip === "focused") return "focused";
  return "calm";
}

function roundRect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  const radius = Math.min(r, w / 2, h / 2);
  ctx.beginPath();
  ctx.moveTo(x + radius, y);
  ctx.arcTo(x + w, y, x + w, y + h, radius);
  ctx.arcTo(x + w, y + h, x, y + h, radius);
  ctx.arcTo(x, y + h, x, y, radius);
  ctx.arcTo(x, y, x + w, y, radius);
  ctx.closePath();
}

function heart(ctx: CanvasRenderingContext2D, x: number, y: number, s: number) {
  ctx.beginPath();
  ctx.moveTo(x, y + s * 0.32);
  ctx.bezierCurveTo(x - s * 0.55, y - s * 0.15, x - s * 0.55, y + s * 0.45, x, y + s * 0.72);
  ctx.bezierCurveTo(x + s * 0.55, y + s * 0.45, x + s * 0.55, y - s * 0.15, x, y + s * 0.32);
  ctx.closePath();
}

function paintGlass(ctx: CanvasRenderingContext2D) {
  ctx.clearRect(0, 0, SIZE, SIZE);
  ctx.fillStyle = "#0a0a0b";
  ctx.fillRect(0, 0, SIZE, SIZE);
  for (let y = 0; y < SIZE; y += 4) {
    ctx.fillStyle = y % 8 === 0 ? "rgba(255,255,255,0.045)" : "rgba(0,0,0,0.25)";
    ctx.fillRect(0, y, SIZE, 1);
  }
  const gloss = ctx.createLinearGradient(40, 20, 280, 360);
  gloss.addColorStop(0, "rgba(190, 210, 230, 0.22)");
  gloss.addColorStop(0.28, "rgba(190, 210, 230, 0.05)");
  gloss.addColorStop(0.55, "rgba(190, 210, 230, 0)");
  ctx.fillStyle = gloss;
  ctx.beginPath();
  ctx.ellipse(150, 120, 120, 70, -0.5, 0, Math.PI * 2);
  ctx.fill();
}

function eyeOpen(ctx: CanvasRenderingContext2D, style: number, cx: number, cy: number, lookX: number, lookY: number, blink: number, shocked: boolean) {
  const shut = Math.min(1, Math.max(0, blink));
  const w = 86;
  const h = 96 * (1 - shut * 0.92);
  if (h < 4) {
    roundRect(ctx, cx - w / 2, cy - 3, w, 6, 3);
    ctx.fill();
    return;
  }
  ctx.save();
  ctx.translate(cx, cy);
  if (shocked) {
    ctx.lineWidth = 14;
    ctx.lineCap = "round";
    ctx.beginPath();
    ctx.moveTo(-28, -28);
    ctx.lineTo(28, 28);
    ctx.moveTo(28, -28);
    ctx.lineTo(-28, 28);
    ctx.stroke();
    ctx.restore();
    return;
  }
  ctx.beginPath();
  if (style === 1) {
    roundRect(ctx, -54, -16, 108, 32, 16);
  } else if (style === 3) {
    heart(ctx, 0, -h * 0.2, h * 0.85);
  } else if (style === 4) {
    ctx.ellipse(0, h * 0.08, w * 0.42, Math.max(6, h * 0.16), 0, 0, Math.PI * 2);
  } else if (style === 5) {
    const cell = 16;
    const grid = [
      [0, 1, 1, 0],
      [1, 1, 1, 1],
      [1, 1, 1, 1],
      [0, 1, 1, 0],
    ];
    grid.forEach((row, iy) => {
      row.forEach((on, ix) => {
        if (!on) return;
        ctx.rect(-32 + ix * cell, -32 * (h / 96) + iy * cell * (h / 96), cell - 2, cell * (h / 96) - 2);
      });
    });
  } else {
    ctx.ellipse(0, 0, w * 0.42, h * 0.46, 0, 0, Math.PI * 2);
  }
  ctx.fill();
  if (style !== 5) {
    ctx.fillStyle = "#0a0a0b";
    const px = lookX * 16;
    const py = lookY * 12;
    if (style === 3) ctx.ellipse(px, py + 8, 11, 11, 0, 0, Math.PI * 2);
    else if (style === 4) ctx.ellipse(px, py + 4, 10, 6, 0, 0, Math.PI * 2);
    else ctx.ellipse(px, py, 14, 16, 0, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = "#ffffff";
    ctx.beginPath();
    ctx.arc(-16 + lookX * 4, -18 + lookY * 3, style === 4 ? 4 : 7, 0, Math.PI * 2);
    ctx.fill();
  } else {
    ctx.fillStyle = "#0a0a0b";
    ctx.fillRect(-8 + lookX * 10, -6 + lookY * 8, 14, 14);
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(-18 + lookX * 4, -18, 8, 8);
  }
  ctx.restore();
}

function brows(ctx: CanvasRenderingContext2D, expression: FaceExpression, blink: number) {
  const lift = expression === "shocked" ? -28 : expression === "happy" ? -16 : expression === "focused" ? 4 : -6;
  const tilt = expression === "focused" ? 0.35 : expression === "happy" ? -0.25 : expression === "shocked" ? -0.05 : -0.12;
  const y = 168 + lift + blink * 18;
  ctx.save();
  ctx.translate(256 - 64, y);
  ctx.rotate(-tilt);
  roundRect(ctx, -40, -6, 78, 14, 7);
  ctx.fill();
  ctx.restore();
  ctx.save();
  ctx.translate(256 + 64, y);
  ctx.rotate(tilt);
  roundRect(ctx, -38, -6, 78, 14, 7);
  ctx.fill();
  ctx.restore();
}

function mouth(ctx: CanvasRenderingContext2D, expression: FaceExpression) {
  ctx.save();
  ctx.translate(256, 338);
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  if (expression === "happy") {
    ctx.lineWidth = 16;
    ctx.beginPath();
    ctx.moveTo(-70, -8);
    ctx.quadraticCurveTo(0, 58, 70, -8);
    ctx.stroke();
  } else if (expression === "shocked") {
    ctx.beginPath();
    ctx.ellipse(0, 8, 28, 36, 0, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = "#0a0a0b";
    ctx.beginPath();
    ctx.ellipse(0, 12, 12, 18, 0, 0, Math.PI * 2);
    ctx.fill();
  } else if (expression === "focused") {
    roundRect(ctx, -36, -5, 72, 12, 6);
    ctx.fill();
  } else {
    ctx.lineWidth = 12;
    ctx.beginPath();
    ctx.moveTo(-32, 0);
    ctx.quadraticCurveTo(0, 22, 32, 0);
    ctx.stroke();
  }
  ctx.restore();
}

function carets(ctx: CanvasRenderingContext2D, lookX: number, blink: number) {
  const drop = blink * 22;
  [-64, 64].forEach((side) => {
    const cx = 256 + side + lookX * 6;
    const top = 172 + drop;
    ctx.beginPath();
    ctx.moveTo(cx - 48, 270);
    ctx.lineTo(cx, top);
    ctx.lineTo(cx + 48, 270);
    ctx.lineTo(cx + 22, 270);
    ctx.lineTo(cx, top + 36);
    ctx.lineTo(cx - 22, 270);
    ctx.closePath();
    ctx.fill();
  });
}

export function redrawFace(
  rig: FaceRig,
  options: { style: number; expression: FaceExpression; blink: number; lookX: number; lookY: number; color: string },
) {
  const glow = rig.glow.getContext("2d");
  if (!glow) return;
  glow.clearRect(0, 0, SIZE, SIZE);
  glow.fillStyle = "#000000";
  glow.fillRect(0, 0, SIZE, SIZE);
  const style = ((options.style % 6) + 6) % 6;
  const expression = options.expression;
  const ink = options.color;
  glow.fillStyle = ink;
  glow.strokeStyle = ink;
  if (expression === "happy") {
    carets(glow, options.lookX, options.blink);
  } else if (style === 2 && expression !== "shocked") {
    const h = 34 * (1 - options.blink * 0.9);
    glow.fillStyle = ink;
    roundRect(glow, 256 - 118, 236 - h / 2, 236, Math.max(6, h), 16);
    glow.fill();
    glow.fillStyle = "#0a0a0b";
    glow.beginPath();
    glow.arc(256 - 52 + options.lookX * 14, 236 + options.lookY * 6, 8, 0, Math.PI * 2);
    glow.arc(256 + 52 + options.lookX * 14, 236 + options.lookY * 6, 8, 0, Math.PI * 2);
    glow.fill();
    glow.fillStyle = "#ffffff";
    glow.beginPath();
    glow.arc(256 - 70, 228, 4, 0, Math.PI * 2);
    glow.arc(256 + 34, 228, 4, 0, Math.PI * 2);
    glow.fill();
  } else if (style === 3 && expression !== "shocked") {
    glow.fillStyle = ink;
    [-64, 64].forEach((side) => {
      heart(glow, 256 + side + options.lookX * 6, 200 + options.lookY * 4, 70 * (1 - options.blink * 0.7));
      glow.fill();
    });
    glow.fillStyle = "#ffffff";
    glow.beginPath();
    glow.arc(256 - 78, 214, 5, 0, Math.PI * 2);
    glow.arc(256 + 50, 214, 5, 0, Math.PI * 2);
    glow.fill();
  } else {
    glow.fillStyle = ink;
    glow.strokeStyle = ink;
    eyeOpen(glow, style, 256 - 64, 236, options.lookX, options.lookY, options.blink, expression === "shocked");
    eyeOpen(glow, style, 256 + 64, 236, options.lookX, options.lookY, options.blink, expression === "shocked");
  }
  glow.fillStyle = ink;
  glow.strokeStyle = ink;
  if (expression !== "happy") brows(glow, expression, options.blink);
  mouth(glow, expression);
  rig.glowTex.needsUpdate = true;
  const tint = rig.plate.material as THREE.MeshStandardMaterial;
  if (tint.emissive) tint.emissive.set(ink);
}

export function mountFace(root: Object3D, read: (node: Object3D) => string): FaceRig | null {
  const found: { plate: THREE.Mesh | null } = { plate: null };
  root.traverse((node) => {
    const mesh = node as THREE.Mesh;
    if (mesh.isMesh && sourceOf(node, read) === "body.faceplate") found.plate = mesh;
  });
  const plate = found.plate;
  if (!plate) return null;
  const color = document.createElement("canvas");
  color.width = SIZE;
  color.height = SIZE;
  const glow = document.createElement("canvas");
  glow.width = SIZE;
  glow.height = SIZE;
  const colorCtx = color.getContext("2d");
  if (colorCtx) paintGlass(colorCtx);
  const colorTex = new THREE.CanvasTexture(color);
  const glowTex = new THREE.CanvasTexture(glow);
  colorTex.colorSpace = THREE.SRGBColorSpace;
  glowTex.colorSpace = THREE.SRGBColorSpace;
  colorTex.anisotropy = 4;
  glowTex.anisotropy = 4;
  const time = { value: 0 };
  const material = new THREE.MeshStandardMaterial({
    name: "Face",
    color: "#0a0a0b",
    map: colorTex,
    emissive: "#ffffff",
    emissiveMap: glowTex,
    emissiveIntensity: 5.5,
    roughness: 0.16,
    metalness: 0.42,
    envMapIntensity: 0.2,
    side: THREE.DoubleSide,
    polygonOffset: true,
    polygonOffsetFactor: -2,
  });
  material.onBeforeCompile = (shader) => {
    shader.uniforms.uTime = time;
    shader.fragmentShader = shader.fragmentShader
      .replace(
        "#include <emissivemap_fragment>",
        `#include <emissivemap_fragment>
         float scan = 0.9 + 0.1 * sin(vMapUv.y * 150.0);
         diffuseColor.rgb *= scan;
         vec2 glossUv = vMapUv - vec2(0.28 + 0.015 * sin(uTime), 0.78);
         float gloss = pow(max(0.0, 1.0 - length(glossUv * vec2(1.7, 2.4))), 7.0);
         totalEmissiveRadiance += vec3(0.45, 0.52, 0.62) * gloss;`,
      )
      .replace("#include <common>", "#include <common>\nuniform float uTime;");
  };
  plate.material = material;
  plate.visible = true;
  plate.renderOrder = 2;
  return { plate, color, glow, colorTex, glowTex, time };
}
