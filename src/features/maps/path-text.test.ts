import Konva from "konva";
import { describe, expect, it } from "vitest";
import {
  distanceToPolyline,
  flattenSpline,
  orientForText,
  pointsToSvgPath,
  polylineLength,
  sliceCentered,
} from "./path-text";

const line = [
  { x: 0, y: 0 },
  { x: 100, y: 0 },
  { x: 100, y: 50 },
];

describe("pointsToSvgPath", () => {
  it("draws straight segments with tension 0", () => {
    expect(pointsToSvgPath(line, 0, false)).toBe("M0 0L100 0L100 50");
  });

  it("closes a closed path", () => {
    expect(pointsToSvgPath(line, 0, true)).toBe("M0 0L100 0L100 50L0 0Z");
  });

  it("returns an empty string for fewer than two points", () => {
    expect(pointsToSvgPath([], 0.5, false)).toBe("");
    expect(pointsToSvgPath([{ x: 1, y: 1 }], 0.5, false)).toBe("");
  });

  it("uses quadratic ends and a cubic middle with tension", () => {
    const pts = [
      { x: 0, y: 0 },
      { x: 100, y: 0 },
      { x: 200, y: 100 },
      { x: 300, y: 100 },
    ];
    const d = pointsToSvgPath(pts, 0.5, false);
    expect(d.startsWith("M0 0Q")).toBe(true);
    expect(d.endsWith("300 100")).toBe(true);
    expect(d.match(/Q/g)).toHaveLength(2);
    expect(d.match(/C/g)).toHaveLength(1);
    expect(d).not.toContain("Z");
  });

  it("stays straight with only two points even with tension", () => {
    const two = [
      { x: 0, y: 0 },
      { x: 10, y: 10 },
    ];
    expect(pointsToSvgPath(two, 0.5, false)).toBe("M0 0L10 10");
  });

  it("makes a closed curve of cubic segments ending in Z", () => {
    const square = [
      { x: 0, y: 0 },
      { x: 100, y: 0 },
      { x: 100, y: 100 },
      { x: 0, y: 100 },
    ];
    const d = pointsToSvgPath(square, 0.5, true);
    expect(d.match(/C/g)).toHaveLength(4);
    expect(d.endsWith("0 0Z")).toBe(true);
  });
});

describe("flattenSpline", () => {
  it("passes through every original point", () => {
    const pts = [
      { x: 0, y: 0 },
      { x: 100, y: 0 },
      { x: 200, y: 100 },
      { x: 300, y: 100 },
    ];
    const flat = flattenSpline(pts, 0.5, false, 10);
    for (const p of pts) {
      expect(flat.some((f) => Math.hypot(f.x - p.x, f.y - p.y) < 1e-9)).toBe(true);
    }
    expect(flat[0]).toEqual(pts[0]);
    expect(flat[flat.length - 1]).toEqual(pts[3]);
  });

  it("returns the points themselves for tension 0", () => {
    expect(flattenSpline(line, 0, false)).toEqual(line);
  });

  it("is longer than the chord when curved", () => {
    const pts = [
      { x: 0, y: 0 },
      { x: 100, y: 80 },
      { x: 200, y: 0 },
    ];
    expect(polylineLength(flattenSpline(pts, 0.5, false))).toBeGreaterThan(200);
  });
});

describe("orientForText", () => {
  it("keeps a left-to-right path", () => {
    expect(orientForText(line, false)).toBe(line);
  });

  it("reverses a right-to-left path so text is not upside down", () => {
    const reversed = line.slice().reverse();
    expect(orientForText(reversed, false)).toEqual(line);
  });

  it("never reverses a closed path or a vertical one", () => {
    const rtl = line.slice().reverse();
    expect(orientForText(rtl, true)).toBe(rtl);
    const vertical = [
      { x: 5, y: 100 },
      { x: 5, y: 0 },
    ];
    expect(orientForText(vertical, false)).toBe(vertical);
  });
});

describe("sliceCentered", () => {
  const straight = [
    { x: 0, y: 0 },
    { x: 100, y: 0 },
  ];

  it("keeps the whole path when the text is longer", () => {
    expect(sliceCentered(straight, 150)).toBe(straight);
  });

  it("centres a shorter text on the path", () => {
    const [a, b] = sliceCentered(straight, 40);
    expect(a).toEqual({ x: 30, y: 0 });
    expect(b).toEqual({ x: 70, y: 0 });
  });

  it("follows bends and keeps corner points inside the slice", () => {
    const slice = sliceCentered(line, 100); // total length 150: from 25 to 125
    expect(slice[0]).toEqual({ x: 25, y: 0 });
    expect(slice).toContainEqual({ x: 100, y: 0 });
    expect(slice[slice.length - 1]).toEqual({ x: 100, y: 25 });
    expect(polylineLength(slice)).toBeCloseTo(100);
  });
});

describe("distanceToPolyline", () => {
  it("measures to the nearest segment", () => {
    expect(distanceToPolyline({ x: 50, y: 10 }, line)).toBe(10);
    expect(distanceToPolyline({ x: 110, y: 25 }, line)).toBe(10);
    expect(distanceToPolyline({ x: -3, y: -4 }, line)).toBe(5);
  });
});

describe("agreement with Konva's own curve", () => {
  /** Rebuilds Konva's segments for a Line as SVG path data, to compare with ours. */
  function konvaPath(points: { x: number; y: number }[], tension: number, closed: boolean) {
    const flat = points.flatMap((p) => [p.x, p.y]);
    // A real `Konva.Line` needs a canvas, which jsdom lacks, so borrow its methods
    // on a plain object that only knows the three settings they read.
    const konvaLine = Object.assign(Object.create(Konva.Line.prototype) as object, {
      points: () => flat,
      closed: () => closed,
      tension: () => tension,
      _getCache: (_key: string, make: () => unknown) => make.call(konvaLine),
    });
    const r = (v: number) => String(Math.round(v * 100) / 100);
    let d = `M${r(flat[0] ?? 0)} ${r(flat[1] ?? 0)}`;
    // `_eachTensionSegment` is what Line itself draws from (quadratic ends, cubic middle).
    (
      konvaLine as unknown as {
        _eachTensionSegment: (q: (...n: number[]) => void, c: (...n: number[]) => void) => void;
      }
    )._eachTensionSegment(
      (_x0, _y0, cx, cy, x, y) => (d += `Q${r(cx!)} ${r(cy!)} ${r(x!)} ${r(y!)}`),
      (_x0, _y0, c1x, c1y, c2x, c2y, x, y) =>
        (d += `C${r(c1x!)} ${r(c1y!)} ${r(c2x!)} ${r(c2y!)} ${r(x!)} ${r(y!)}`),
    );
    return closed ? `${d}Z` : d;
  }

  const wiggle = [
    { x: 10, y: 20 },
    { x: 80, y: 60 },
    { x: 150, y: 10 },
    { x: 230, y: 90 },
    { x: 300, y: 40 },
  ];

  it("matches an open Line with tension", () => {
    for (const n of [3, 4, 5]) {
      const pts = wiggle.slice(0, n);
      expect(pointsToSvgPath(pts, 0.5, false)).toBe(konvaPath(pts, 0.5, false));
    }
  });

  it("matches a closed Line with tension", () => {
    expect(pointsToSvgPath(wiggle, 0.4, true)).toBe(konvaPath(wiggle, 0.4, true));
  });
});
