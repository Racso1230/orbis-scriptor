import { describe, expect, it } from "vitest";
import { simplify, simplifyToMax } from "./rdp";

describe("simplify (Ramer–Douglas–Peucker)", () => {
  it("returns short inputs unchanged (as a copy)", () => {
    const pts = [{ x: 0, y: 0 }];
    expect(simplify(pts, 1)).toEqual(pts);
    expect(simplify(pts, 1)).not.toBe(pts);
  });

  it("drops points that lie on a straight line", () => {
    const line = Array.from({ length: 50 }, (_, i) => ({ x: i, y: i * 2 }));
    expect(simplify(line, 0.5)).toEqual([line[0], line[49]]);
  });

  it("keeps a corner that sticks out further than epsilon, drops a near-flat one", () => {
    const peak = [
      { x: 0, y: 0 },
      { x: 10, y: 10 },
      { x: 20, y: 0 },
    ];
    expect(simplify(peak, 1)).toEqual(peak);
    const flat = [
      { x: 0, y: 0 },
      { x: 10, y: 0.1 },
      { x: 20, y: 0 },
    ];
    expect(simplify(flat, 1)).toEqual([flat[0], flat[2]]);
  });

  it("keeps endpoints and handles a closed loop of identical ends", () => {
    const pts = [
      { x: 0, y: 0 },
      { x: 10, y: 10 },
      { x: 0, y: 0 },
    ];
    expect(simplify(pts, 1)).toEqual(pts);
  });

  it("handles very long strokes without recursion limits", () => {
    const pts = Array.from({ length: 100000 }, (_, i) => ({ x: i, y: Math.sin(i / 50) * 40 }));
    const out = simplify(pts, 0.5);
    expect(out.length).toBeLessThan(pts.length);
    expect(out[0]).toEqual(pts[0]);
    expect(out[out.length - 1]).toEqual(pts[pts.length - 1]);
  });

  it("simplifyToMax enforces the point budget", () => {
    const pts = Array.from({ length: 5000 }, (_, i) => ({ x: i, y: Math.sin(i / 7) * 30 }));
    const out = simplifyToMax(pts, 0.1, 100);
    expect(out.length).toBeLessThanOrEqual(100);
    expect(out.length).toBeGreaterThanOrEqual(2);
  });
});
