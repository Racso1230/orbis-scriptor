import { describe, expect, it } from "vitest";
import {
  MAX_ZOOM,
  MIN_ZOOM,
  centerOn,
  fitToScreen,
  mapToScreen,
  screenToMap,
  zoomAround,
} from "./viewport";

describe("coordinate conversion", () => {
  it("round-trips screen and map points", () => {
    const t = { scale: 0.5, x: 100, y: -40 };
    const m = screenToMap(t, { x: 300, y: 60 });
    expect(m).toEqual({ x: 400, y: 200 });
    expect(mapToScreen(t, m)).toEqual({ x: 300, y: 60 });
  });
});

describe("zoomAround", () => {
  it("keeps the map point under the cursor fixed", () => {
    const t = { scale: 0.8, x: 50, y: 20 };
    const pointer = { x: 400, y: 300 };
    const before = screenToMap(t, pointer);
    for (const delta of [-100, 100]) {
      const next = zoomAround(t, pointer, delta);
      const after = screenToMap(next, pointer);
      expect(after.x).toBeCloseTo(before.x, 6);
      expect(after.y).toBeCloseTo(before.y, 6);
    }
  });

  it("zooms in on negative delta and out on positive", () => {
    const t = { scale: 1, x: 0, y: 0 };
    expect(zoomAround(t, { x: 0, y: 0 }, -1).scale).toBeGreaterThan(1);
    expect(zoomAround(t, { x: 0, y: 0 }, 1).scale).toBeLessThan(1);
  });

  it("clamps to the zoom limits and stays anchored", () => {
    const pointer = { x: 10, y: 10 };
    const max = zoomAround({ scale: MAX_ZOOM, x: 0, y: 0 }, pointer, -1);
    expect(max.scale).toBe(MAX_ZOOM);
    const min = zoomAround({ scale: MIN_ZOOM, x: 5, y: 5 }, pointer, 1);
    expect(min.scale).toBe(MIN_ZOOM);
    expect(min.x).toBeCloseTo(5, 6);
  });

  it("ignores a zero delta", () => {
    const t = { scale: 2, x: 1, y: 2 };
    expect(zoomAround(t, { x: 5, y: 5 }, 0)).toBe(t);
  });
});

describe("fitToScreen", () => {
  it("contains a wide map and centres it vertically", () => {
    const t = fitToScreen({ width: 1000, height: 500 }, { width: 600, height: 600 }, 50);
    expect(t.scale).toBeCloseTo(0.5);
    expect(t.x).toBeCloseTo(50);
    expect(t.y).toBeCloseTo(175);
  });

  it("contains a tall map by height", () => {
    const t = fitToScreen({ width: 100, height: 1000 }, { width: 500, height: 300 }, 0);
    expect(t.scale).toBeCloseTo(0.3);
    expect(t.x).toBeCloseTo(235);
  });

  it("falls back to identity for an empty map", () => {
    expect(fitToScreen({ width: 0, height: 0 }, { width: 100, height: 100 })).toEqual({
      scale: 1,
      x: 0,
      y: 0,
    });
  });
});

describe("centerOn", () => {
  it("puts the target in the middle of the container", () => {
    const t = centerOn({ x: 100, y: 200 }, { width: 800, height: 600 }, 2);
    expect(mapToScreen(t, { x: 100, y: 200 })).toEqual({ x: 400, y: 300 });
  });
});
