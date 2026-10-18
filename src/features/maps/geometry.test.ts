import { describe, expect, it } from "vitest";
import type { MapGeometry } from "@/lib/bindings";
import {
  boundsOf,
  flattenPoints,
  moveVertex,
  pinGeometry,
  pinPoint,
  translateGeometry,
} from "./geometry";

describe("pinPoint", () => {
  it("returns the point for a valid pin", () => {
    expect(pinPoint({ kind: "pin", x: 10, y: 20 })).toEqual({ x: 10, y: 20 });
  });

  it("rejects non-finite coordinates", () => {
    expect(pinPoint({ kind: "pin", x: NaN, y: 5 })).toBeNull();
    expect(pinPoint({ kind: "pin", x: 5, y: NaN })).toBeNull();
    expect(pinPoint({ kind: "pin", x: Infinity, y: 5 })).toBeNull();
  });

  it("returns null for shapes that are not pins", () => {
    expect(pinPoint({ kind: "stamp", x: 1, y: 2, icon: "city", scale: 1, rotation: 0 })).toBeNull();
    expect(pinPoint({ kind: "area", points: [] })).toBeNull();
  });

  it("round-trips through pinGeometry", () => {
    expect(pinPoint(pinGeometry({ x: 1.5, y: 2.5 }))).toEqual({ x: 1.5, y: 2.5 });
  });
});

describe("shape helpers", () => {
  const path: MapGeometry = {
    kind: "path",
    points: [
      { x: 0, y: 0 },
      { x: 10, y: 5 },
    ],
    closed: false,
  };

  it("translates paths, areas and single-point kinds without mutating", () => {
    const moved = translateGeometry(path, 3, -2);
    expect(moved).toEqual({
      kind: "path",
      points: [
        { x: 3, y: -2 },
        { x: 13, y: 3 },
      ],
      closed: false,
    });
    expect(path.kind === "path" && path.points[0]).toEqual({ x: 0, y: 0 });
    expect(translateGeometry({ kind: "pin", x: 1, y: 2 }, 1, 1)).toEqual({
      kind: "pin",
      x: 2,
      y: 3,
    });
    const label: MapGeometry = { kind: "label", x: 1, y: 1, text: "A", rotation: 5, pathId: null };
    expect(translateGeometry(label, 2, 2)).toMatchObject({ x: 3, y: 3, text: "A", rotation: 5 });
  });

  it("moves one vertex and ignores bad indexes", () => {
    expect(moveVertex(path, 1, { x: 7, y: 7 })).toMatchObject({
      points: [
        { x: 0, y: 0 },
        { x: 7, y: 7 },
      ],
    });
    expect(moveVertex(path, 5, { x: 7, y: 7 })).toBe(path);
  });

  it("computes bounds and flattens points", () => {
    expect(boundsOf([])).toBeNull();
    expect(
      boundsOf([
        { x: 1, y: 5 },
        { x: -2, y: 9 },
      ]),
    ).toEqual({ minX: -2, minY: 5, maxX: 1, maxY: 9 });
    expect(
      flattenPoints([
        { x: 1, y: 2 },
        { x: 3, y: 4 },
      ]),
    ).toEqual([1, 2, 3, 4]);
  });
});
