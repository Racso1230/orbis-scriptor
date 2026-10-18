import { describe, expect, it } from "vitest";
import { pinGeometry, pinPoint } from "./geometry";

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
