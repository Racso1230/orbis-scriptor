import { describe, expect, it } from "vitest";
import {
  FREEHAND_MAX_POINTS,
  appendPoint,
  canFinish,
  freehandToPoints,
  isNearFirst,
  pathLength,
} from "./drawing";

describe("polyline drawing helpers", () => {
  it("ignores a point on top of the previous one (double-click second click)", () => {
    const one = appendPoint([], { x: 5, y: 5 });
    expect(appendPoint(one, { x: 5.1, y: 5 })).toBe(one);
    expect(appendPoint(one, { x: 9, y: 5 })).toHaveLength(2);
  });

  it("detects the first point in screen pixels, not map pixels", () => {
    const pts = [
      { x: 0, y: 0 },
      { x: 100, y: 0 },
      { x: 100, y: 100 },
    ];
    // 4 map px away: 4 screen px at zoom 1 (close), 40 at zoom 10 (not close).
    expect(isNearFirst(pts, { x: 4, y: 0 }, 1)).toBe(true);
    expect(isNearFirst(pts, { x: 4, y: 0 }, 10)).toBe(false);
    expect(isNearFirst([], { x: 0, y: 0 }, 1)).toBe(false);
  });

  it("needs 2 points for a path and 3 for an area", () => {
    const two = [
      { x: 0, y: 0 },
      { x: 1, y: 1 },
    ];
    expect(canFinish("path", two)).toBe(true);
    expect(canFinish("area", two)).toBe(false);
    expect(canFinish("area", [...two, { x: 2, y: 0 }])).toBe(true);
  });

  it("measures length", () => {
    expect(
      pathLength([
        { x: 0, y: 0 },
        { x: 3, y: 4 },
        { x: 3, y: 10 },
      ]),
    ).toBe(11);
  });
});

describe("freehandToPoints", () => {
  it("rejects a click or tiny wobble", () => {
    expect(freehandToPoints([{ x: 1, y: 1 }], 1)).toBeNull();
    expect(
      freehandToPoints(
        [
          { x: 1, y: 1 },
          { x: 2, y: 1 },
        ],
        1,
      ),
    ).toBeNull();
  });

  it("reduces a dense stroke to a sane number of points and keeps its ends", () => {
    const raw = Array.from({ length: 3000 }, (_, i) => ({ x: i, y: Math.sin(i / 30) * 50 }));
    const out = freehandToPoints(raw, 1);
    expect(out).not.toBeNull();
    expect(out?.length).toBeLessThan(raw.length / 4);
    expect(out?.length).toBeLessThanOrEqual(FREEHAND_MAX_POINTS);
    expect(out?.[0]).toEqual(raw[0]);
    expect(out?.[out.length - 1]).toEqual(raw[raw.length - 1]);
  });

  it("is more tolerant (fewer points) when zoomed out", () => {
    const raw = Array.from({ length: 800 }, (_, i) => ({ x: i * 4, y: Math.sin(i / 5) * 20 }));
    const zoomedIn = freehandToPoints(raw, 2);
    const zoomedOut = freehandToPoints(raw, 0.25);
    expect(zoomedOut && zoomedIn && zoomedOut.length < zoomedIn.length).toBe(true);
  });
});
