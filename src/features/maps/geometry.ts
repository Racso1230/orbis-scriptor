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

/** Points of a path or area, or `null` for the other kinds. */
export function shapePoints(geometry: MapGeometry): Point[] | null {
  return geometry.kind === "path" || geometry.kind === "area" ? geometry.points : null;
}

/** The anchor of any single-point kind (pin, label, stamp), or `null`. */
export function anchorPoint(geometry: MapGeometry): Point | null {
  if (geometry.kind === "pin" || geometry.kind === "label" || geometry.kind === "stamp") {
    return { x: geometry.x, y: geometry.y };
  }
  return null;
}

/**
 * Moves a geometry by (dx, dy). Single-point kinds move their x/y, paths and
 * areas move every point. Returns a new object (geometry is never mutated).
 */
export function translateGeometry(geometry: MapGeometry, dx: number, dy: number): MapGeometry {
  switch (geometry.kind) {
    case "path":
    case "area":
      return { ...geometry, points: geometry.points.map((p) => ({ x: p.x + dx, y: p.y + dy })) };
    default:
      return { ...geometry, x: geometry.x + dx, y: geometry.y + dy };
  }
}

/** Replaces one vertex of a path/area. Other kinds are returned unchanged. */
export function moveVertex(geometry: MapGeometry, index: number, to: Point): MapGeometry {
  if (geometry.kind !== "path" && geometry.kind !== "area") return geometry;
  if (index < 0 || index >= geometry.points.length) return geometry;
  const points = geometry.points.map((p, i) => (i === index ? { x: to.x, y: to.y } : p));
  return { ...geometry, points };
}

export interface Bounds {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
}

/** Bounding box of a list of points, or `null` when empty. */
export function boundsOf(points: Point[]): Bounds | null {
  const first = points[0];
  if (!first) return null;
  const b: Bounds = { minX: first.x, minY: first.y, maxX: first.x, maxY: first.y };
  for (const p of points) {
    b.minX = Math.min(b.minX, p.x);
    b.minY = Math.min(b.minY, p.y);
    b.maxX = Math.max(b.maxX, p.x);
    b.maxY = Math.max(b.maxY, p.y);
  }
  return b;
}

/** Flattens points into Konva's `[x0, y0, x1, y1, ...]` format. */
export function flattenPoints(points: Point[]): number[] {
  const out: number[] = [];
  for (const p of points) out.push(p.x, p.y);
  return out;
}

export function distance(a: Point, b: Point): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

/** A good point to centre the view on for any object ("jump to" links). */
export function focusPoint(geometry: MapGeometry): Point | null {
  const anchor = anchorPoint(geometry);
  if (anchor) return Number.isFinite(anchor.x) && Number.isFinite(anchor.y) ? anchor : null;
  const points = shapePoints(geometry);
  const b = points ? boundsOf(points) : null;
  return b ? { x: (b.minX + b.maxX) / 2, y: (b.minY + b.maxY) / 2 } : null;
}
