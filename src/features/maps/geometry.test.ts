import { describe, expect, it } from "vitest";
import { pinGeometry, pinPoint } from "./geometry";

describe("pinPoint", () => {
  it("returns the point for a valid pin", () => {
    expect(pinPoint({ kind: "pin", x: 10, y: 20 })).toEqual({ x: 10, y: 20 });
  });

  it("rejects null or non-finite coordinates", () => {
    expect(pinPoint({ kind: "pin", x: null, y: 5 })).toBeNull();
    expect(pinPoint({ kind: "pin", x: 5, y: null })).toBeNull();
    expect(pinPoint({ kind: "pin", x: Infinity, y: 5 })).toBeNull();
  });

  it("round-trips through pinGeometry", () => {
    expect(pinPoint(pinGeometry({ x: 1.5, y: 2.5 }))).toEqual({ x: 1.5, y: 2.5 });
  });
});
