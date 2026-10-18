import type { BackgroundKind, MapBackground } from "@/lib/bindings";

/** Default fill for the "plain" background when no colour is stored. */
export const DEFAULT_PLAIN_COLOR = "#f5f5f4";

/** Longest side of the generated texture; Konva stretches it to the map size. */
export const MAX_TEXTURE_SIDE = 2048;

export const BACKGROUND_LABELS: Record<BackgroundKind, string> = {
  parchment: "Parchment",
  ocean: "Ocean",
  plain: "Plain colour",
};

/** Texture size for a map: same aspect ratio, longest side capped. */
export function textureSize(width: number, height: number): { width: number; height: number } {
  const longest = Math.max(width, height, 1);
  const k = Math.min(1, MAX_TEXTURE_SIDE / longest);
  return { width: Math.max(1, Math.round(width * k)), height: Math.max(1, Math.round(height * k)) };
}

/** Cache key: textures only depend on kind, tint and (capped) size. */
export function backgroundKey(bg: MapBackground, width: number, height: number): string {
  const t = textureSize(width, height);
  return `${bg.kind}|${bg.color ?? ""}|${t.width}x${t.height}`;
}

/** A small, fast, seedable random generator (so a texture is identical every time). */
function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Adds per-pixel brightness noise. `amount` is the +/- range in 0..255 units. */
function addNoise(
  ctx: CanvasRenderingContext2D,
  w: number,
  h: number,
  amount: number,
  seed: number,
) {
  const rand = mulberry32(seed);
  const image = ctx.getImageData(0, 0, w, h);
  const d = image.data;
  for (let i = 0; i < d.length; i += 4) {
    const n = (rand() - 0.5) * amount;
    d[i] = (d[i] ?? 0) + n;
    d[i + 1] = (d[i + 1] ?? 0) + n;
    d[i + 2] = (d[i + 2] ?? 0) + n;
  }
  ctx.putImageData(image, 0, 0);
}

function vignette(ctx: CanvasRenderingContext2D, w: number, h: number, strength: number) {
  const r = Math.hypot(w, h) / 2;
  const g = ctx.createRadialGradient(w / 2, h / 2, r * 0.45, w / 2, h / 2, r);
  g.addColorStop(0, "rgba(60, 35, 10, 0)");
  g.addColorStop(1, `rgba(60, 35, 10, ${strength})`);
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, w, h);
}

function tint(ctx: CanvasRenderingContext2D, w: number, h: number, color: string, alpha: number) {
  ctx.save();
  ctx.globalCompositeOperation = "multiply";
  ctx.globalAlpha = alpha;
  ctx.fillStyle = color;
  ctx.fillRect(0, 0, w, h);
  ctx.restore();
}

function paintParchment(ctx: CanvasRenderingContext2D, w: number, h: number, color: string | null) {
  const rand = mulberry32(1234);
  const base = ctx.createRadialGradient(w / 2, h / 2, 0, w / 2, h / 2, Math.hypot(w, h) / 2);
  base.addColorStop(0, "#f3e5bf");
  base.addColorStop(1, "#d9c18c");
  ctx.fillStyle = base;
  ctx.fillRect(0, 0, w, h);

  // Soft stains.
  const side = Math.max(w, h);
  for (let i = 0; i < 90; i++) {
    const x = rand() * w;
    const y = rand() * h;
    const r = (0.04 + rand() * 0.14) * side;
    const g = ctx.createRadialGradient(x, y, 0, x, y, r);
    const dark = rand() < 0.6;
    g.addColorStop(0, dark ? "rgba(120, 80, 30, 0.07)" : "rgba(255, 245, 215, 0.08)");
    g.addColorStop(1, "rgba(0, 0, 0, 0)");
    ctx.fillStyle = g;
    ctx.fillRect(x - r, y - r, r * 2, r * 2);
  }
  // Paper fibres.
  ctx.lineWidth = Math.max(0.5, side / 2500);
  for (let i = 0; i < 700; i++) {
    const x = rand() * w;
    const y = rand() * h;
    const len = (0.01 + rand() * 0.03) * side;
    const angle = rand() * Math.PI;
    ctx.strokeStyle = `rgba(110, 75, 35, ${0.03 + rand() * 0.05})`;
    ctx.beginPath();
    ctx.moveTo(x, y);
    ctx.lineTo(x + Math.cos(angle) * len, y + Math.sin(angle) * len);
    ctx.stroke();
  }
  addNoise(ctx, w, h, 14, 99);
  if (color) tint(ctx, w, h, color, 0.45);
  vignette(ctx, w, h, 0.45);
}

function paintOcean(ctx: CanvasRenderingContext2D, w: number, h: number, color: string | null) {
  const rand = mulberry32(4321);
  const base = ctx.createLinearGradient(0, 0, w, h);
  base.addColorStop(0, "#2f78ad");
  base.addColorStop(1, "#1e5686");
  ctx.fillStyle = base;
  ctx.fillRect(0, 0, w, h);

  // Short curling wave strokes.
  const side = Math.max(w, h);
  ctx.lineCap = "round";
  ctx.lineWidth = Math.max(0.8, side / 1200);
  for (let i = 0; i < 1400; i++) {
    const x = rand() * w;
    const y = rand() * h;
    const len = (0.008 + rand() * 0.02) * side;
    ctx.strokeStyle = `rgba(255, 255, 255, ${0.04 + rand() * 0.07})`;
    ctx.beginPath();
    ctx.moveTo(x, y);
    ctx.quadraticCurveTo(x + len / 2, y - len * 0.3, x + len, y);
    ctx.stroke();
  }
  addNoise(ctx, w, h, 10, 7);
  if (color) tint(ctx, w, h, color, 0.45);
}

const cache = new Map<string, HTMLCanvasElement>();
const MAX_CACHED = 4;

/**
 * Procedurally generated texture for parchment and ocean maps, drawn once into
 * an offscreen canvas and cached (so panning and zooming never regenerate it).
 * Returns `null` for `plain` (the caller draws a rectangle) or when canvas is
 * unavailable (tests). Stretch the result to the map's width and height.
 */
export function getBackgroundCanvas(
  bg: MapBackground,
  width: number,
  height: number,
): HTMLCanvasElement | null {
  if (bg.kind === "plain" || typeof document === "undefined") return null;
  const key = backgroundKey(bg, width, height);
  const hit = cache.get(key);
  if (hit) return hit;

  const size = textureSize(width, height);
  const canvas = document.createElement("canvas");
  canvas.width = size.width;
  canvas.height = size.height;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;
  if (bg.kind === "parchment") paintParchment(ctx, size.width, size.height, bg.color);
  else paintOcean(ctx, size.width, size.height, bg.color);

  if (cache.size >= MAX_CACHED) {
    const oldest = cache.keys().next().value;
    if (oldest !== undefined) cache.delete(oldest);
  }
  cache.set(key, canvas);
  return canvas;
}
