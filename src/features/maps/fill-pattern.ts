import type { FillPattern } from "./object-style";
import { isHexColor, withOpacity } from "./object-style";

const cache = new Map<string, HTMLCanvasElement>();
const TILE = 16;

/**
 * A small repeating tile for hatch / dot area fills, with a faint solid
 * background so the area still reads as filled. Cached per (pattern, colour,
 * opacity). Returns `null` where canvas is unavailable (tests).
 */
export function getPatternTile(
  pattern: Exclude<FillPattern, "none">,
  color: string,
  opacity: number,
): HTMLCanvasElement | null {
  if (typeof document === "undefined") return null;
  const safe = isHexColor(color) ? color : "#000000";
  const key = `${pattern}|${safe}|${opacity.toFixed(2)}`;
  const hit = cache.get(key);
  if (hit) return hit;

  const canvas = document.createElement("canvas");
  canvas.width = TILE;
  canvas.height = TILE;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;

  ctx.fillStyle = withOpacity(safe, opacity * 0.4);
  ctx.fillRect(0, 0, TILE, TILE);
  const ink = withOpacity(safe, Math.min(1, opacity * 2 + 0.2));
  if (pattern === "hatch") {
    ctx.strokeStyle = ink;
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    // Three segments so the diagonal also covers the tile corners seamlessly.
    ctx.moveTo(-2, TILE + 2);
    ctx.lineTo(TILE + 2, -2);
    ctx.moveTo(-2, 2 * TILE + 2);
    ctx.lineTo(2 * TILE + 2, -2);
    ctx.moveTo(-TILE - 2, TILE + 2);
    ctx.lineTo(TILE + 2, -TILE - 2);
    ctx.stroke();
  } else {
    ctx.fillStyle = ink;
    ctx.beginPath();
    ctx.arc(TILE / 2, TILE / 2, 2, 0, Math.PI * 2);
    ctx.fill();
  }
  cache.set(key, canvas);
  return canvas;
}
