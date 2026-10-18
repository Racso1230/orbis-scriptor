import type { Point } from "./geometry";

/** Distance from point `p` to the segment a-b. */
function segmentDistance(p: Point, a: Point, b: Point): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const lengthSq = dx * dx + dy * dy;
  if (lengthSq === 0) return Math.hypot(p.x - a.x, p.y - a.y);
  const t = Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / lengthSq));
  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
}

/**
 * Ramer–Douglas–Peucker line simplification: keeps the first and last point
 * and any point that sticks out more than `epsilon` from the straight line
 * between the points kept on either side of it.
 *
 * Written with an explicit stack instead of recursion, so a long freehand
 * stroke (tens of thousands of samples) cannot overflow the call stack.
 */
export function simplify(points: Point[], epsilon: number): Point[] {
  if (points.length <= 2) return points.slice();
  const keep = new Array<boolean>(points.length).fill(false);
  keep[0] = true;
  keep[points.length - 1] = true;
  const stack: [number, number][] = [[0, points.length - 1]];
  while (stack.length > 0) {
    const range = stack.pop();
    if (!range) break;
    const [start, end] = range;
    const a = points[start];
    const b = points[end];
    if (!a || !b) continue;
    let farthest = -1;
    let maxDistance = epsilon;
    for (let i = start + 1; i < end; i++) {
      const p = points[i];
      if (!p) continue;
      const d = segmentDistance(p, a, b);
      if (d > maxDistance) {
        maxDistance = d;
        farthest = i;
      }
    }
    if (farthest !== -1) {
      keep[farthest] = true;
      stack.push([start, farthest], [farthest, end]);
    }
  }
  return points.filter((_, i) => keep[i]);
}

/**
 * Simplifies with `epsilon`, then keeps doubling it until at most
 * `maxPoints` remain (the backend allows 20000 but a hand-drawn river
 * looks fine with far fewer).
 */
export function simplifyToMax(points: Point[], epsilon: number, maxPoints: number): Point[] {
  let result = simplify(points, epsilon);
  let e = Math.max(epsilon, 0.001);
  // Bounded loop: each pass doubles epsilon, so it ends quickly.
  for (let i = 0; i < 40 && result.length > maxPoints; i++) {
    e *= 2;
    result = simplify(points, e);
  }
  return result.length > maxPoints ? result.slice(0, maxPoints) : result;
}
