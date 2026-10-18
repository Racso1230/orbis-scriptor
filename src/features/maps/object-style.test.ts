import { describe, expect, it } from "vitest";
import {
  areaStyle,
  dashFor,
  dashPresetOf,
  lineStyle,
  PATH_PRESETS,
  withOpacity,
} from "./object-style";

describe("style readers", () => {
  it("falls back to defaults for missing or invalid values", () => {
    expect(lineStyle({})).toEqual({ stroke: "#334155", strokeWidth: 4, dash: [], smoothing: 0 });
    expect(lineStyle({ stroke: "red", strokeWidth: "x", dash: "no", smoothing: 5 })).toMatchObject({
      stroke: "#334155",
      strokeWidth: 4,
      dash: [],
      smoothing: 1,
    });
  });

  it("ignores a dash array that would not draw anything", () => {
    expect(lineStyle({ dash: [0, 0] }).dash).toEqual([]);
    expect(lineStyle({ dash: [4, -1, "a"] }).dash).toEqual([4]);
  });

  it("reads the built-in presets as intended", () => {
    expect(lineStyle(PATH_PRESETS.river.style)).toMatchObject({
      stroke: "#3b82f6",
      strokeWidth: 6,
      smoothing: 0.5,
    });
    expect(lineStyle(PATH_PRESETS.road.style).dash.length).toBeGreaterThan(0);
    expect(lineStyle(PATH_PRESETS.border.style)).toMatchObject({ strokeWidth: 3 });
  });

  it("areas default to a 25% fill and take their stroke from the fill", () => {
    const a = areaStyle({ fill: "#112233" });
    expect(a).toMatchObject({ fill: "#112233", stroke: "#112233", fillOpacity: 0.25 });
    expect(a.fillPattern).toBe("none");
    expect(areaStyle({ fillPattern: "hatch" }).fillPattern).toBe("hatch");
    expect(areaStyle({ fillPattern: "stripes" }).fillPattern).toBe("none");
  });
});

describe("dash presets", () => {
  it("round-trips solid, dashed and dotted", () => {
    expect(dashPresetOf(dashFor("solid", 4))).toBe("solid");
    expect(dashPresetOf(dashFor("dashed", 4))).toBe("dashed");
    expect(dashPresetOf(dashFor("dotted", 4))).toBe("dotted");
  });
});

describe("withOpacity", () => {
  it("converts hex to rgba", () => {
    expect(withOpacity("#ff8000", 0.5)).toBe("rgba(255, 128, 0, 0.5)");
    expect(withOpacity("nope", 0.5)).toBe("rgba(0, 0, 0, 0.5)");
  });
});
