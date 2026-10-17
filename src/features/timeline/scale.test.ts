import { describe, expect, it } from "vitest";
import {
  MAX_DAYS_PER_PIXEL,
  MIN_DAYS_PER_PIXEL,
  dayToX,
  fitRange,
  panBy,
  queryWindow,
  visibleRange,
  xToDay,
  zoomAround,
  type TimelineViewport,
} from "./scale";

const v: TimelineViewport = { centerDay: 100, daysPerPixel: 2 };

describe("coordinates", () => {
  it("puts the centre day in the middle and round-trips", () => {
    expect(dayToX(v, 1000, 100)).toBe(500);
    expect(dayToX(v, 1000, 110)).toBe(505);
    expect(xToDay(v, 1000, dayToX(v, 1000, 321))).toBeCloseTo(321);
  });

  it("reports the visible range", () => {
    expect(visibleRange(v, 1000)).toEqual({ from: -900, to: 1100 });
  });
});

describe("panBy", () => {
  it("moves the centre opposite to the drag", () => {
    expect(panBy(v, 50).centerDay).toBe(0);
    expect(panBy(v, -50).centerDay).toBe(200);
  });
});

describe("zoomAround", () => {
  it("keeps the day under the cursor fixed", () => {
    const before = xToDay(v, 1000, 800);
    const next = zoomAround(v, 1000, 800, -1);
    expect(next.daysPerPixel).toBeLessThan(v.daysPerPixel);
    expect(xToDay(next, 1000, 800)).toBeCloseTo(before);
  });

  it("zooms out on positive delta and ignores zero", () => {
    expect(zoomAround(v, 1000, 500, 1).daysPerPixel).toBeGreaterThan(2);
    expect(zoomAround(v, 1000, 500, 0)).toBe(v);
  });

  it("clamps at both ends", () => {
    let t = v;
    for (let i = 0; i < 200; i++) t = zoomAround(t, 1000, 500, -1);
    expect(t.daysPerPixel).toBe(MIN_DAYS_PER_PIXEL);
    for (let i = 0; i < 200; i++) t = zoomAround(t, 1000, 500, 1);
    expect(t.daysPerPixel).toBe(MAX_DAYS_PER_PIXEL);
  });
});

describe("fitRange", () => {
  it("shows the whole range inside the padding", () => {
    const fit = fitRange(0, 999, 1000, 0);
    expect(fit.centerDay).toBe(500);
    expect(fit.daysPerPixel).toBe(1);
    const x0 = dayToX(fit, 1000, 0);
    const x1 = dayToX(fit, 1000, 1000);
    expect(x0).toBeGreaterThanOrEqual(0);
    expect(x1).toBeLessThanOrEqual(1000);
  });

  it("clamps a single day to the maximum zoom-in", () => {
    expect(fitRange(5, 5, 1000).daysPerPixel).toBe(MIN_DAYS_PER_PIXEL);
  });
});

describe("queryWindow", () => {
  it("covers the requested range", () => {
    const w = queryWindow(-300, 700);
    expect(w.from).toBeLessThanOrEqual(-300);
    expect(w.to).toBeGreaterThanOrEqual(700);
  });

  it("is stable for small pans so the query key does not change", () => {
    const a = queryWindow(1000, 2000);
    const b = queryWindow(1010, 2010);
    expect(b).toEqual(a);
  });

  it("changes after a large pan", () => {
    expect(queryWindow(1000, 2000)).not.toEqual(queryWindow(9000, 10000));
  });
});
