import { describe, expect, it } from "vitest";
import { assignLanes, eventExtent, POINT_RADIUS } from "./lanes";

const laneOf = (items: Parameters<typeof assignLanes>[0], gap?: number) =>
  Object.fromEntries(assignLanes(items, gap).map((a) => [a.id, a.lane]));

describe("assignLanes", () => {
  it("returns nothing for no items", () => {
    expect(assignLanes([])).toEqual([]);
  });

  it("keeps non-overlapping events in lane 0", () => {
    const lanes = laneOf([
      { id: "a", start: 0, end: 50 },
      { id: "b", start: 100, end: 150 },
      { id: "c", start: 200, end: 210 },
    ]);
    expect(lanes).toEqual({ a: 0, b: 0, c: 0 });
  });

  it("stacks overlapping events into separate lanes", () => {
    const lanes = laneOf([
      { id: "a", start: 0, end: 100 },
      { id: "b", start: 50, end: 120 },
      { id: "c", start: 60, end: 70 },
    ]);
    expect(lanes).toEqual({ a: 0, b: 1, c: 2 });
  });

  it("reuses the lowest free lane", () => {
    const lanes = laneOf([
      { id: "war", start: 0, end: 500 },
      { id: "p1", start: 10, end: 60 },
      { id: "p2", start: 100, end: 150 },
    ]);
    // p1 sits in lane 1; p2 starts after p1 ended, so it drops back into lane 1, not 2.
    expect(lanes).toEqual({ war: 0, p1: 1, p2: 1 });
  });

  it("honours the gap between neighbours", () => {
    const items = [
      { id: "a", start: 0, end: 100 },
      { id: "b", start: 104, end: 150 },
    ];
    expect(laneOf(items, 6)).toEqual({ a: 0, b: 1 });
    expect(laneOf(items, 4)).toEqual({ a: 0, b: 0 });
  });

  it("sorts by start and is stable for ties", () => {
    const result = assignLanes([
      { id: "late", start: 200, end: 210 },
      { id: "x", start: 0, end: 100 },
      { id: "y", start: 0, end: 100 },
    ]);
    expect(result.map((r) => r.id)).toEqual(["x", "y", "late"]);
    expect(result.map((r) => r.lane)).toEqual([0, 1, 0]);
  });
});

describe("eventExtent", () => {
  it("reserves marker and label room for a point", () => {
    const e = eventExtent(100, null, "Coronation");
    expect(e.start).toBe(100 - POINT_RADIUS);
    expect(e.end).toBeGreaterThan(100 + POINT_RADIUS + e.labelWidth);
  });

  it("draws the label inside a wide bar", () => {
    const e = eventExtent(0, 400, "War");
    expect(e.labelInside).toBe(true);
    expect(e.start).toBe(0);
    expect(e.end).toBe(400);
  });

  it("extends a narrow bar to fit its label after it", () => {
    const e = eventExtent(0, 10, "The Long War");
    expect(e.labelInside).toBe(false);
    expect(e.end).toBeGreaterThan(10 + e.labelWidth);
  });
});
