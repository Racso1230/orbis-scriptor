import { describe, expect, it } from "vitest";
import { pinLook } from "./pin-style";

describe("pinLook", () => {
  it("is grey and marked for unlinked pins", () => {
    expect(pinLook({ entityId: null, entityType: null })).toEqual({
      color: "#78716c",
      glyph: "?",
    });
  });

  it("uses a distinct look per entity type", () => {
    const loc = pinLook({ entityId: "1", entityType: "location" });
    const chr = pinLook({ entityId: "2", entityType: "character" });
    expect(loc.color).not.toBe(chr.color);
    expect(loc.glyph).toBe("L");
  });

  it("falls back to a generic look for unknown types", () => {
    expect(pinLook({ entityId: "1", entityType: "weird" }).glyph).toBe("•");
  });
});

describe("pinLook with a type look", () => {
  it("prefers the colour from the type definition", () => {
    const look = pinLook(
      { entityId: "1", entityType: "custom-deity" },
      { color: "#a855f7", glyph: "D" },
    );
    expect(look).toEqual({ color: "#a855f7", glyph: "D" });
  });

  it("stays grey when unlinked even with a type look", () => {
    expect(
      pinLook({ entityId: null, entityType: null }, { color: "#a855f7", glyph: "D" }).glyph,
    ).toBe("?");
  });
});
