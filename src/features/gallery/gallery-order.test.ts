import { describe, expect, it } from "vitest";
import { hasPendingThumbs, moveId, stepIndex, thumbOrFull } from "./gallery-order";

describe("moveId", () => {
  it("moves an item up or down", () => {
    expect(moveId(["a", "b", "c"], 1, -1)).toEqual(["b", "a", "c"]);
    expect(moveId(["a", "b", "c"], 1, 1)).toEqual(["a", "c", "b"]);
  });

  it("clamps at the ends and does not change the input", () => {
    const ids = ["a", "b", "c"];
    expect(moveId(ids, 0, -1)).toEqual(["a", "b", "c"]);
    expect(moveId(ids, 2, 1)).toEqual(["a", "b", "c"]);
    expect(ids).toEqual(["a", "b", "c"]);
  });
});

describe("thumbnails", () => {
  it("prefers the thumbnail", () => {
    expect(thumbOrFull({ path: "full", thumbPath: "thumb" })).toBe("thumb");
    expect(thumbOrFull({ path: "full", thumbPath: null })).toBe("full");
  });

  it("detects images still waiting for a thumbnail", () => {
    expect(hasPendingThumbs([{ thumbPath: "t" }, { thumbPath: null }])).toBe(true);
    expect(hasPendingThumbs([{ thumbPath: "t" }])).toBe(false);
    expect(hasPendingThumbs([])).toBe(false);
  });
});

describe("stepIndex", () => {
  it("wraps around in both directions", () => {
    expect(stepIndex(0, -1, 3)).toBe(2);
    expect(stepIndex(2, 1, 3)).toBe(0);
    expect(stepIndex(1, 1, 3)).toBe(2);
    expect(stepIndex(0, 1, 0)).toBe(0);
  });
});
