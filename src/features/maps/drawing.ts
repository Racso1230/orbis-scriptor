import { distance, type Point } from "./geometry";
import { simplifyToMax } from "./rdp";

export type PolylineKind = "path" | "area";

/** Fewest points a finished shape may have (the backend enforces the same). */
export const MIN_POINTS: Record<PolylineKind, number> = { path: 2, area: 3 };

/** A freehand stroke is cut down to at most this many points before saving. */
export const FREEHAND_MAX_POINTS = 400;

/** How close (screen px) a click must be to the first point to close an area. */
export const CLOSE_RADIUS_PX = 10;

/** Smallest freehand stroke (screen px of travel) that counts as a line, not a click. */
export const MIN_STROKE_PX = 6;

/**
 * Adds a clicked point unless it lands on top of the previous one (which is
 * what the second click of a double-click does). Returns the same array when
 * nothing was added.
 */
export function appendPoint(points: Point[], p: Point, minDistance = 0.5): Point[] {
  const last = points[points.length - 1];
  if (last && distance(last, p) < minDistance) return points;
  return [...points, p];
}

/** Whether `p` is within `radiusPx` screen pixels of the first point at this zoom. */
export function isNearFirst(points: Point[], p: Point, scale: number, radiusPx = CLOSE_RADIUS_PX) {
  const first = points[0];
  return first !== undefined && distance(first, p) * scale <= radiusPx;
}

/** True when `points` is enough for a finished path / area. */
export function canFinish(kind: PolylineKind, points: Point[]): boolean {
  return points.length >= MIN_POINTS[kind];
}

/** Total length of a polyline. */
export function pathLength(points: Point[]): number {
  let total = 0;
  for (let i = 1; i < points.length; i++) {
    const a = points[i - 1];
    const b = points[i];
    if (a && b) total += distance(a, b);
  }
  return total;
}

/**
 * Turns the raw samples of a freehand stroke into the points to save:
 * Ramer–Douglas–Peucker with a tolerance of ~1.5 screen pixels (so the
 * result looks the same at the zoom it was drawn at), capped at
 * `FREEHAND_MAX_POINTS`. Returns `null` for a stroke too short to be a line.
 */
export function freehandToPoints(raw: Point[], scale: number): Point[] | null {
  if (raw.length < 2 || pathLength(raw) * scale < MIN_STROKE_PX) return null;
  const simplified = simplifyToMax(raw, 1.5 / scale, FREEHAND_MAX_POINTS);
  return simplified.length >= 2 ? simplified : null;
}
