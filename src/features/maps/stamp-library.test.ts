import { describe, expect, it } from "vitest";
import {
  STAMPS,
  STAMP_CATEGORIES,
  defaultStampScale,
  fillColor,
  mixHex,
  stampByKey,
  stampName,
  stampPalette,
  stampsIn,
} from "./stamp-library";

/** Keys the backend sample world and dev seeder already use. */
const REQUIRED = ["mountain", "forest", "city", "castle", "tower"];

describe("stamp library", () => {
  it("has 16 to 24 stamps with unique keys the backend accepts", () => {
    expect(STAMPS.length).toBeGreaterThanOrEqual(16);
    expect(STAMPS.length).toBeLessThanOrEqual(24);
    const keys = STAMPS.map((s) => s.key);
    expect(new Set(keys).size).toBe(keys.length);
    for (const key of keys) expect(key).toMatch(/^[a-z0-9-]{1,64}$/);
  });

  it("includes every key the sample world uses", () => {
    for (const key of REQUIRED) expect(stampByKey(key)).toBeDefined();
  });

  it("puts every stamp in a known category with a name", () => {
    const categories = new Set(STAMP_CATEGORIES.map((c) => c.id));
    for (const s of STAMPS) {
      expect(categories.has(s.category)).toBe(true);
      expect(s.name.trim()).not.toBe("");
    }
    expect(STAMP_CATEGORIES.flatMap((c) => stampsIn(c.id))).toHaveLength(STAMPS.length);
  });

  it("only uses path data made of valid commands and numbers", () => {
    // Commands, numbers and separators only: every letter must be a command Konva parses.
    const valid = /^[MmLlHhVvCcSsQqTtAaZz0-9.\s,-]+$/;
    for (const s of STAMPS) {
      expect(s.shapes.length).toBeGreaterThan(0);
      for (const shape of s.shapes) {
        expect(shape.d, `${s.key}: ${shape.d}`).toMatch(valid);
        expect(shape.d.trimStart()[0], s.key).toMatch(/[Mm]/);
      }
    }
  });

  it("keeps every coordinate roughly inside the 100 x 100 box", () => {
    for (const s of STAMPS) {
      for (const shape of s.shapes) {
        for (const num of shape.d.match(/-?\d+(\.\d+)?/g) ?? []) {
          expect(Math.abs(Number(num)), `${s.key}: ${shape.d}`).toBeLessThanOrEqual(110);
        }
      }
    }
  });

  it("falls back to the key for an unknown stamp", () => {
    expect(stampByKey("dragon-nest")).toBeUndefined();
    expect(stampName("dragon-nest")).toBe("dragon-nest");
    expect(stampName("castle")).toBe("Castle");
  });
});

describe("stamp colours and size", () => {
  it("uses the default ink without a tint and the tint as ink with one", () => {
    expect(stampPalette(null).ink).toBe("#2b2118");
    expect(stampPalette("nonsense").ink).toBe("#2b2118");
    const tinted = stampPalette("#aa0000");
    expect(tinted.ink).toBe("#aa0000");
    expect(tinted.paper).not.toBe(stampPalette(null).paper);
  });

  it("mixes hex colours", () => {
    expect(mixHex("#000000", "#ffffff", 0.5)).toBe("#808080");
    expect(mixHex("#102030", "#102030", 0.9)).toBe("#102030");
  });

  it("resolves fills, keeping none as none", () => {
    expect(fillColor("none", stampPalette(null))).toBe("none");
    expect(fillColor("ink", stampPalette("#123456"))).toBe("#123456");
  });

  it("starts bigger stamps on bigger maps", () => {
    expect(defaultStampScale(800)).toBe(0.5);
    expect(defaultStampScale(1600)).toBe(1);
    expect(defaultStampScale(4000)).toBe(2.5);
    expect(defaultStampScale(1_000_000)).toBe(8);
  });
});
