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
const CX = 256;
/** Eyes sit above the screen centre. The pair's outer edges span 55% of the opening. */
const EYE_Y = 206;
const EYE_DX = 78;
const EYE_RX = 62;
const MOUTH_Y = 338;

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
    ctx.fillStyle = y % 8 === 0 ? "rgba(255,255,255,0.03)" : "rgba(0,0,0,0.16)";
    ctx.fillRect(0, y, SIZE, 1);
  }
  ctx.strokeStyle = "rgba(214, 226, 238, 0.55)";
  ctx.lineWidth = 3;
  ctx.lineCap = "round";
  ctx.beginPath();
  ctx.moveTo(118, 78);
  ctx.lineTo(168, 118);
  ctx.stroke();
}

function eyeOpen(ctx: CanvasRenderingContext2D, style: number, cx: number, cy: number, lookX: number, lookY: number, blink: number, shocked: boolean) {
  const shut = Math.min(1, Math.max(0, blink));
  const w = EYE_RX * 2;
  const h = 118 * (1 - shut * 0.92);
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
    ctx.moveTo(-46, -46);
    ctx.lineTo(46, 46);
    ctx.moveTo(46, -46);
    ctx.lineTo(-46, 46);
    ctx.stroke();
    ctx.restore();
    return;
  }
  ctx.beginPath();
  if (style === 1) {
    roundRect(ctx, -EYE_RX, -20, EYE_RX * 2, 40, 20);
  } else if (style === 3) {
    heart(ctx, 0, -h * 0.2, h * 0.85);
  } else if (style === 4) {
    ctx.ellipse(0, h * 0.08, w * 0.42, Math.max(6, h * 0.16), 0, 0, Math.PI * 2);
  } else if (style === 5) {
    const cell = (EYE_RX * 2) / 4;
    const grid = [
      [0, 1, 1, 0],
      [1, 1, 1, 1],
      [1, 1, 1, 1],
      [0, 1, 1, 0],
    ];
    const scaleY = h / 124;
    grid.forEach((row, iy) => {
      row.forEach((on, ix) => {
        if (!on) return;
        ctx.rect(-EYE_RX + ix * cell, -EYE_RX * scaleY + iy * cell * scaleY, cell - 2, cell * scaleY - 2);
      });
    });
  } else {
    ctx.ellipse(0, 0, EYE_RX, Math.max(20, h * 0.48), 0, 0, Math.PI * 2);
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
  const lift = expression === "shocked" ? -18 : expression === "focused" ? 8 : 0;
  const tilt = expression === "focused" ? 0.4 : expression === "shocked" ? -0.08 : -0.14;
  const y = EYE_Y - 78 + lift + blink * 16;
  ctx.save();
  ctx.translate(CX - EYE_DX, y);
  ctx.rotate(-tilt);
  roundRect(ctx, -46, -7, 92, 14, 7);
  ctx.fill();
  ctx.restore();
  ctx.save();
  ctx.translate(CX + EYE_DX, y);
  ctx.rotate(tilt);
  roundRect(ctx, -46, -7, 92, 14, 7);
  ctx.fill();
  ctx.restore();
}

function mouth(ctx: CanvasRenderingContext2D, expression: FaceExpression) {
  ctx.save();
  ctx.translate(CX, MOUTH_Y);
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
  [-EYE_DX, EYE_DX].forEach((side) => {
    const cx = CX + side + lookX * 6;
    const top = EYE_Y - 48 + drop;
    ctx.beginPath();
    const foot = EYE_Y + 42;
    ctx.moveTo(cx - EYE_RX, foot);
    ctx.lineTo(cx, top);
    ctx.lineTo(cx + EYE_RX, foot);
    ctx.lineTo(cx + 26, foot);
    ctx.lineTo(cx, top + 42);
    ctx.lineTo(cx - 26, foot);
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
    const h = 46 * (1 - options.blink * 0.9);
    glow.fillStyle = ink;
    roundRect(glow, CX - (EYE_DX + EYE_RX), EYE_Y - h / 2, (EYE_DX + EYE_RX) * 2, Math.max(8, h), 18);
    glow.fill();
    glow.fillStyle = "#0a0a0b";
    glow.beginPath();
    glow.arc(CX - EYE_DX + options.lookX * 14, EYE_Y + options.lookY * 6, 10, 0, Math.PI * 2);
    glow.arc(CX + EYE_DX + options.lookX * 14, EYE_Y + options.lookY * 6, 10, 0, Math.PI * 2);
    glow.fill();
    glow.fillStyle = "#ffffff";
    glow.beginPath();
    glow.arc(CX - EYE_DX - 22, EYE_Y - 10, 5, 0, Math.PI * 2);
    glow.arc(CX + EYE_DX - 22, EYE_Y - 10, 5, 0, Math.PI * 2);
    glow.fill();
  } else if (style === 3 && expression !== "shocked") {
    glow.fillStyle = ink;
    [-EYE_DX, EYE_DX].forEach((side) => {
      heart(glow, CX + side + options.lookX * 6, EYE_Y - 40 + options.lookY * 4, 112 * (1 - options.blink * 0.7));
      glow.fill();
    });
    glow.fillStyle = "#ffffff";
    glow.beginPath();
    glow.arc(CX - EYE_DX - 16, EYE_Y - 8, 6, 0, Math.PI * 2);
    glow.arc(CX + EYE_DX - 16, EYE_Y - 8, 6, 0, Math.PI * 2);
    glow.fill();
  } else {
    glow.fillStyle = ink;
    glow.strokeStyle = ink;
    eyeOpen(glow, style, CX - EYE_DX, EYE_Y, options.lookX, options.lookY, options.blink, expression === "shocked");
    eyeOpen(glow, style, CX + EYE_DX, EYE_Y, options.lookX, options.lookY, options.blink, expression === "shocked");
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
  // Canvas y grows downward. The plate's UV v grows upward, so leave the upload unflipped.
  colorTex.flipY = false;
  glowTex.flipY = false;
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
    emissiveIntensity: 2.6,
    roughness: 0.72,
    metalness: 0.05,
    envMapIntensity: 0.04,
    side: THREE.DoubleSide,
  });
  material.onBeforeCompile = (shader) => {
    shader.fragmentShader = shader.fragmentShader.replace(
      "#include <emissivemap_fragment>",
      `#include <emissivemap_fragment>
       diffuseColor.rgb *= 0.92 + 0.08 * sin(vMapUv.y * 150.0);`,
    );
  };
  plate.material = material;
  plate.visible = true;
  plate.renderOrder = 2;
  return { plate, color, glow, colorTex, glowTex, time };
}
