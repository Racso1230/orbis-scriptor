import { describe, expect, it } from "vitest";
import {
  matchesTagFilter,
  normalizeTagName,
  suggestTags,
  tagColorMap,
  tagMatches,
  validateTagName,
} from "./tag-utils";

describe("normalizeTagName", () => {
  it("trims and strips leading # characters", () => {
    expect(normalizeTagName("  #royalty ")).toBe("royalty");
    expect(normalizeTagName("##places/cities")).toBe("places/cities");
    expect(normalizeTagName("# spaced")).toBe("spaced");
    expect(normalizeTagName("#")).toBe("");
  });

  it("keeps a # inside the name for validation to reject", () => {
    expect(normalizeTagName("a#b")).toBe("a#b");
  });
});

describe("validateTagName", () => {
  it("accepts letters, digits, - _ spaces and nesting", () => {
    expect(validateTagName("royalty")).toBeNull();
    expect(validateTagName("Éire 2-b_c")).toBeNull();
    expect(validateTagName("places/big cities")).toBeNull();
  });

  it("rejects empty names, empty segments, odd characters and long names", () => {
    expect(validateTagName("")).toMatch(/needs a name/);
    expect(validateTagName("a//b")).toMatch(/empty parts/);
    expect(validateTagName("/a")).toMatch(/empty parts/);
    expect(validateTagName("a/")).toMatch(/empty parts/);
    expect(validateTagName("a#b")).toMatch(/letters, digits/);
    expect(validateTagName("x".repeat(41))).toMatch(/at most 40/);
    expect(validateTagName("x".repeat(40))).toBeNull();
  });
});

describe("tag matching", () => {
  it("matches the tag itself and nested children, ignoring case", () => {
    expect(tagMatches("places/cities", "places")).toBe(true);
    expect(tagMatches("Places", "places")).toBe(true);
    expect(tagMatches("places", "places/cities")).toBe(false);
    expect(tagMatches("placesmith", "places")).toBe(false);
  });

  it("any needs one selected tag, all needs every one", () => {
    const tags = ["coast", "places/cities"];
    expect(matchesTagFilter(tags, ["coast", "royalty"], false)).toBe(true);
    expect(matchesTagFilter(tags, ["coast", "royalty"], true)).toBe(false);
    expect(matchesTagFilter(tags, ["places", "coast"], true)).toBe(true);
    expect(matchesTagFilter(tags, [], true)).toBe(true);
    expect(matchesTagFilter([], ["coast"], false)).toBe(false);
  });
});

describe("suggestTags and colours", () => {
  const all = ["coast", "royalty", "places/cities", "Coastal-defence"];

  it("suggests prefix matches first, skipping tags already used", () => {
    expect(suggestTags(all, "co", [])).toEqual(["coast", "Coastal-defence"]);
    expect(suggestTags(all, "ities", [])).toEqual(["places/cities"]);
    expect(suggestTags(all, "#co", ["COAST"])).toEqual(["Coastal-defence"]);
  });

  it("builds a lower-case colour lookup that skips tags without a colour", () => {
    const map = tagColorMap([
      { id: "1", name: "Coast", color: "#112233", count: 1 },
      { id: "2", name: "royalty", color: null, count: 1 },
    ]);
    expect(map.get("coast")).toBe("#112233");
    expect(map.has("royalty")).toBe(false);
  });
});
