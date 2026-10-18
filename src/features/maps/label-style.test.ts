import { describe, expect, it } from "vitest";
import {
  LABEL_DARK,
  LABEL_LIGHT,
  cleanLabelText,
  defaultLabelStyle,
  haloColor,
  labelLook,
  measureText,
} from "./label-style";

describe("defaultLabelStyle", () => {
  it("scales the font with the map but never below 16", () => {
    expect(defaultLabelStyle(600, null)["fontSize"]).toBe(16);
    expect(defaultLabelStyle(960, null)["fontSize"]).toBe(16);
    expect(defaultLabelStyle(2400, null)["fontSize"]).toBe(40);
    expect(defaultLabelStyle(12000, null)["fontSize"]).toBe(200);
  });

  it("uses dark brown on parchment, light plain colours and image maps", () => {
    expect(defaultLabelStyle(2000, { kind: "parchment", color: null })["color"]).toBe(LABEL_DARK);
    expect(defaultLabelStyle(2000, { kind: "plain", color: "#f5f5f4" })["color"]).toBe(LABEL_DARK);
    expect(defaultLabelStyle(2000, null)["color"]).toBe(LABEL_DARK);
  });

  it("uses a light colour on ocean and dark plain colours", () => {
    expect(defaultLabelStyle(2000, { kind: "ocean", color: null })["color"]).toBe(LABEL_LIGHT);
    expect(defaultLabelStyle(2000, { kind: "plain", color: "#1e293b" })["color"]).toBe(LABEL_LIGHT);
  });
});

describe("labelLook", () => {
  it("falls back to safe defaults", () => {
    const look = labelLook({});
    expect(look.fontSize).toBe(24);
    expect(look.fontKey).toBe("serif");
    expect(look.color).toBe(LABEL_DARK);
    expect(look.letterSpacing).toBe(0);
  });

  it("reads the seeded sample style and ignores junk", () => {
    const look = labelLook({ fontSize: 20, fontFamily: "serif", color: "#1a365d" });
    expect(look).toMatchObject({ fontSize: 20, fontKey: "serif", color: "#1a365d" });
    expect(labelLook({ fontSize: "huge", color: "red" })).toMatchObject({
      fontSize: 24,
      color: LABEL_DARK,
    });
    expect(labelLook({ fontSize: 100000 }).fontSize).toBe(400);
  });

  it("keeps an unknown font-family string as a custom font", () => {
    const look = labelLook({ fontFamily: "Papyrus, fantasy" });
    expect(look.fontKey).toBe("custom");
    expect(look.fontFamily).toBe("Papyrus, fantasy");
  });
});

describe("haloColor and text helpers", () => {
  it("picks a halo that contrasts with the text", () => {
    expect(haloColor("#000000")).toContain("255, 250, 235");
    expect(haloColor("#ffffff")).toContain("20, 15, 10");
  });

  it("cleans label text to one trimmed line of at most 200 characters", () => {
    expect(cleanLabelText("  River\nAshen \r\n")).toBe("River Ashen");
    expect(cleanLabelText("x".repeat(300))).toHaveLength(200);
    expect(cleanLabelText("   ")).toBe("");
  });

  it("estimates width without a canvas and adds letter spacing", () => {
    const plain = measureText("abcd", 10, "serif");
    expect(plain).toBeGreaterThan(0);
    expect(measureText("abcd", 10, "serif", 2)).toBeCloseTo(plain + 6);
  });
});
