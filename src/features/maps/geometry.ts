import type { MapGeometry } from "@/lib/bindings";

export interface Point {
  x: number;
  y: number;
}

/**
 * Returns the position of a pin, or `null` for any other shape (paths, areas,
 * labels, stamps) or for non-finite coordinates.
 */
export function pinPoint(geometry: MapGeometry): Point | null {
  if (geometry.kind !== "pin") return null;
  const { x, y } = geometry;
  if (!Number.isFinite(x) || !Number.isFinite(y)) return null;
  return { x, y };
}

export function pinGeometry(point: Point): MapGeometry {
  return { kind: "pin", x: point.x, y: point.y };
}
