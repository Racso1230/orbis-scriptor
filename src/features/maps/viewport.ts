import type { Point } from "./geometry";

/**
 * How the map is placed inside the stage: a map point `m` appears on screen at
 * `m * scale + (x, y)`. This is exactly Konva's stage scale and position.
 */
export interface Transform {
  scale: number;
  x: number;
  y: number;
}

export interface Size {
  width: number;
  height: number;
}

export const MIN_ZOOM = 0.02;
export const MAX_ZOOM = 8;
/** Each wheel notch multiplies (or divides) the scale by this. */
export const ZOOM_STEP = 1.1;
export const FIT_PADDING = 24;

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export function screenToMap(t: Transform, screen: Point): Point {
  return { x: (screen.x - t.x) / t.scale, y: (screen.y - t.y) / t.scale };
}

export function mapToScreen(t: Transform, map: Point): Point {
  return { x: map.x * t.scale + t.x, y: map.y * t.scale + t.y };
}

/**
 * Zoom one wheel step around the pointer so the map point under the cursor
 * stays under the cursor. Negative `deltaY` (wheel up) zooms in.
 */
export function zoomAround(
  t: Transform,
  pointer: Point,
  deltaY: number,
  min = MIN_ZOOM,
  max = MAX_ZOOM,
): Transform {
  if (deltaY === 0) return t;
  const target = deltaY < 0 ? t.scale * ZOOM_STEP : t.scale / ZOOM_STEP;
  const scale = clamp(target, min, max);
  const anchor = screenToMap(t, pointer);
  return { scale, x: pointer.x - anchor.x * scale, y: pointer.y - anchor.y * scale };
}

/** Largest scale that keeps the whole map visible, centred, with padding. */
export function fitToScreen(
  map: Size,
  container: Size,
  padding = FIT_PADDING,
  min = MIN_ZOOM,
  max = MAX_ZOOM,
): Transform {
  if (map.width <= 0 || map.height <= 0) return { scale: 1, x: 0, y: 0 };
  const availW = Math.max(1, container.width - padding * 2);
  const availH = Math.max(1, container.height - padding * 2);
  const scale = clamp(Math.min(availW / map.width, availH / map.height), min, max);
  return {
    scale,
    x: (container.width - map.width * scale) / 2,
    y: (container.height - map.height * scale) / 2,
  };
}

/** Transform that puts `target` (map coords) in the middle of the container. */
export function centerOn(target: Point, container: Size, scale: number): Transform {
  return {
    scale,
    x: container.width / 2 - target.x * scale,
    y: container.height / 2 - target.y * scale,
  };
}
