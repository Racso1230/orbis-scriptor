import { distance, type Point } from "./geometry";

/**
 * Geometry for text that follows a path.
 *
 * `Line` in Konva draws a "cardinal spline" when `tension > 0`. To lay text
 * along the same curve, the label renderer needs the curve as SVG path data
 * (for Konva's `TextPath`) or as a dense polyline (to trim and centre it).
 * Both come from the same list of segments, built here, so they always agree
 * with each other and with how `PathRenderer` draws the line.
 */

type Segment =
  | { kind: "L"; from: Point; to: Point }
  | { kind: "Q"; from: Point; c1: Point; to: Point }
  | { kind: "C"; from: Point; c1: Point; c2: Point; to: Point };

/**
 * The two spline control points around `cur`: one on the side of `prev`, one
 * on the side of `next`. Same formula as Konva's `Util.getControlPoints`.
 */
function controlPair(prev: Point, cur: Point, next: Point, tension: number): [Point, Point] {
  const d01 = distance(prev, cur);
  const d12 = distance(cur, next);
  const sum = d01 + d12;
  if (sum === 0) return [cur, cur];
  const fa = (tension * d01) / sum;
  const fb = (tension * d12) / sum;
  const dx = next.x - prev.x;
  const dy = next.y - prev.y;
  return [
    { x: cur.x - fa * dx, y: cur.y - fa * dy },
    { x: cur.x + fb * dx, y: cur.y + fb * dy },
  ];
}

/** Breaks a path into straight / curved segments, mirroring Konva's `Line` with `tension`. */
function buildSegments(points: Point[], tension: number, closed: boolean): Segment[] {
  const n = points.length;
  if (n < 2) return [];
  const at = (i: number): Point => points[((i % n) + n) % n] as Point;
  const smooth = tension > 0 && n >= 3;

  if (!smooth) {
    const segments: Segment[] = [];
    const last = closed ? n : n - 1;
    for (let i = 0; i < last; i++) segments.push({ kind: "L", from: at(i), to: at(i + 1) });
    return segments;
  }

  if (closed) {
    const segments: Segment[] = [];
    for (let i = 0; i < n; i++) {
      const [, after] = controlPair(at(i - 1), at(i), at(i + 1), tension);
      const [before] = controlPair(at(i), at(i + 1), at(i + 2), tension);
      segments.push({ kind: "C", from: at(i), c1: after, c2: before, to: at(i + 1) });
    }
    return segments;
  }

  // Open: the first and last segment are quadratic (a single control point, which is
  // the neighbouring point's own control point), the ones in between are cubic.
  const segments: Segment[] = [];
  const beforeOf = (i: number) => controlPair(at(i - 1), at(i), at(i + 1), tension)[0];
  const afterOf = (i: number) => controlPair(at(i - 1), at(i), at(i + 1), tension)[1];
  segments.push({ kind: "Q", from: at(0), c1: beforeOf(1), to: at(1) });
  for (let i = 1; i < n - 2; i++) {
    segments.push({ kind: "C", from: at(i), c1: afterOf(i), c2: beforeOf(i + 1), to: at(i + 1) });
  }
  segments.push({ kind: "Q", from: at(n - 2), c1: afterOf(n - 2), to: at(n - 1) });
  return segments;
}

function fmt(n: number): string {
  return String(Math.round(n * 100) / 100);
}

function pt(p: Point): string {
  return `${fmt(p.x)} ${fmt(p.y)}`;
}

/**
 * SVG path data (`M … L … Q … C … Z`) for a path's points, curved the same way
 * Konva curves a `Line` with this `tension` (0 = straight segments).
 */
export function pointsToSvgPath(points: Point[], tension: number, closed: boolean): string {
  const segments = buildSegments(points, tension, closed);
  const first = segments[0];
  if (!first) return "";
  let d = `M${pt(first.from)}`;
  for (const s of segments) {
    if (s.kind === "L") d += `L${pt(s.to)}`;
    else if (s.kind === "Q") d += `Q${pt(s.c1)} ${pt(s.to)}`;
    else d += `C${pt(s.c1)} ${pt(s.c2)} ${pt(s.to)}`;
  }
  return closed ? `${d}Z` : d;
}

function evaluate(s: Segment, t: number): Point {
  if (s.kind === "L") {
    return { x: s.from.x + (s.to.x - s.from.x) * t, y: s.from.y + (s.to.y - s.from.y) * t };
  }
  const u = 1 - t;
  if (s.kind === "Q") {
    return {
      x: u * u * s.from.x + 2 * u * t * s.c1.x + t * t * s.to.x,
      y: u * u * s.from.y + 2 * u * t * s.c1.y + t * t * s.to.y,
    };
  }
  return {
    x: u * u * u * s.from.x + 3 * u * u * t * s.c1.x + 3 * u * t * t * s.c2.x + t * t * t * s.to.x,
    y: u * u * u * s.from.y + 3 * u * u * t * s.c1.y + 3 * u * t * t * s.c2.y + t * t * t * s.to.y,
  };
}

/** The curve as a dense polyline (`steps` samples per curved segment). */
export function flattenSpline(
  points: Point[],
  tension: number,
  closed: boolean,
  steps = 12,
): Point[] {
  const segments = buildSegments(points, tension, closed);
  const first = segments[0];
  if (!first) return [];
  const out: Point[] = [first.from];
  for (const s of segments) {
    const count = s.kind === "L" ? 1 : steps;
    for (let i = 1; i <= count; i++) out.push(evaluate(s, i / count));
  }
  return out;
}

/** Length of a polyline. */
export function polylineLength(points: Point[]): number {
  let total = 0;
  for (let i = 1; i < points.length; i++) {
    const a = points[i - 1];
    const b = points[i];
    if (a && b) total += distance(a, b);
  }
  return total;
}

/**
 * Text must read left to right: a path drawn right-to-left (end is left of
 * the start) is reversed so the letters are not upside down. Closed paths
 * keep their direction.
 */
export function orientForText(points: Point[], closed: boolean): Point[] {
  const first = points[0];
  const last = points[points.length - 1];
  if (closed || !first || !last || last.x >= first.x) return points;
  return points.slice().reverse();
}

/**
 * The part of a polyline that a text of `textLength` covers when centred on
 * it. If the text is as long as the polyline (or longer) the whole polyline is
 * returned and the text simply runs off its end.
 */
export function sliceCentered(polyline: Point[], textLength: number): Point[] {
  const total = polylineLength(polyline);
  if (polyline.length < 2 || textLength >= total) return polyline;
  const start = (total - textLength) / 2;
  const end = start + textLength;
  const out: Point[] = [];
  let walked = 0;
  for (let i = 1; i < polyline.length; i++) {
    const a = polyline[i - 1];
    const b = polyline[i];
    if (!a || !b) continue;
    const len = distance(a, b);
    const next = walked + len;
    if (len > 0 && next >= start && walked <= end) {
      const t0 = Math.max(0, (start - walked) / len);
      const t1 = Math.min(1, (end - walked) / len);
      if (out.length === 0) out.push({ x: a.x + (b.x - a.x) * t0, y: a.y + (b.y - a.y) * t0 });
      out.push({ x: a.x + (b.x - a.x) * t1, y: a.y + (b.y - a.y) * t1 });
    }
    walked = next;
    if (walked > end) break;
  }
  return out.length >= 2 ? out : polyline;
}

/** Shortest distance from `p` to a polyline. */
export function distanceToPolyline(p: Point, polyline: Point[]): number {
  let best = Infinity;
  for (let i = 1; i < polyline.length; i++) {
    const a = polyline[i - 1];
    const b = polyline[i];
    if (!a || !b) continue;
    const dx = b.x - a.x;
    const dy = b.y - a.y;
    const lengthSq = dx * dx + dy * dy;
    const t =
      lengthSq === 0
        ? 0
        : Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / lengthSq));
    best = Math.min(best, Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy)));
  }
  return best;
}
