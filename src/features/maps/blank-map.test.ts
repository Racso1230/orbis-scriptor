import { describe, expect, it } from "vitest";
import { validateBlankMap, type BlankMapForm } from "./blank-map";
import { backgroundKey, textureSize } from "./map-background";

const form = (over: Partial<BlankMapForm> = {}): BlankMapForm => ({
  size: "small",
  customWidth: "",
  customHeight: "",
  background: { kind: "parchment", color: null },
  ...over,
});

describe("validateBlankMap", () => {
  it("accepts a preset", () => {
    expect(validateBlankMap(form())).toEqual({
      ok: true,
      value: { width: 2048, height: 1536, background: { kind: "parchment", color: null } },
    });
  });

  it("accepts custom sizes at the edges (256 and 16384)", () => {
    for (const [w, h] of [
      ["256", "256"],
      ["16384", "16384"],
    ] as const) {
      const r = validateBlankMap(form({ size: "custom", customWidth: w, customHeight: h }));
      expect(r.ok).toBe(true);
    }
  });

  it("rejects custom sizes out of range or not numeric", () => {
    for (const [w, h] of [
      ["255", "500"],
      ["500", "16385"],
      ["abc", "500"],
      ["", ""],
      ["12.5", "500"],
    ] as const) {
      const r = validateBlankMap(form({ size: "custom", customWidth: w, customHeight: h }));
      expect(r.ok).toBe(false);
      if (!r.ok) expect(r.errors.size).toBeTruthy();
    }
  });

  it("requires a valid colour for plain maps and validates tints", () => {
    const plain = validateBlankMap(form({ background: { kind: "plain", color: null } }));
    expect(plain.ok).toBe(false);
    const bad = validateBlankMap(form({ background: { kind: "ocean", color: "blue" } }));
    expect(bad.ok).toBe(false);
    const good = validateBlankMap(form({ background: { kind: "plain", color: "#ffffff" } }));
    expect(good.ok).toBe(true);
    const tinted = validateBlankMap(form({ background: { kind: "ocean", color: "#336699" } }));
    expect(tinted.ok).toBe(true);
  });
});

describe("background texture sizing", () => {
  it("caps the texture but keeps the aspect ratio", () => {
    expect(textureSize(1000, 500)).toEqual({ width: 1000, height: 500 });
    expect(textureSize(16384, 8192)).toEqual({ width: 2048, height: 1024 });
  });

  it("keys textures by kind, tint and capped size", () => {
    const a = backgroundKey({ kind: "ocean", color: null }, 4096, 3072);
    expect(a).toBe(backgroundKey({ kind: "ocean", color: null }, 4096, 3072));
    expect(a).not.toBe(backgroundKey({ kind: "ocean", color: "#112233" }, 4096, 3072));
    expect(a).not.toBe(backgroundKey({ kind: "parchment", color: null }, 4096, 3072));
  });
});
