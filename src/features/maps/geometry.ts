import type { MapGeometry } from "@/lib/bindings";

export interface Point {
  x: number;
  y: number;
}

/**
 * specta types the coordinates of a pin as `number | null` (Rust `f64` can be
 * NaN/infinite, which JSON cannot carry). This guard narrows them to real numbers.
 */
export function pinPoint(geometry: MapGeometry): Point | null {
  if (geometry.kind !== "pin") return null;
  const { x, y } = geometry;
  if (typeof x !== "number" || typeof y !== "number") return null;
  if (!Number.isFinite(x) || !Number.isFinite(y)) return null;
  return { x, y };
}

export function pinGeometry(point: Point): MapGeometry {
  return { kind: "pin", x: point.x, y: point.y };
}
