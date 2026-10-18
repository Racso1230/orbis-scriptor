import { describe, expect, it } from "vitest";
import { activeTagToken, applyTagSuggestion, formatTagFilter, matchTagNames } from "./tag-token";

describe("activeTagToken", () => {
  it("finds a tag filter being typed at the end", () => {
    expect(activeTagToken("tag:")).toEqual({ start: 0, partial: "" });
    expect(activeTagToken("dragon tag:roy")).toEqual({ start: 7, partial: "roy" });
    expect(activeTagToken("TAG:Roy")).toEqual({ start: 0, partial: "Roy" });
    expect(activeTagToken('tag:"two wo')).toEqual({ start: 0, partial: "two wo" });
  });

  it("ignores finished filters and other words", () => {
    expect(activeTagToken("tag:royalty ")).toBeNull();
    expect(activeTagToken("dragon")).toBeNull();
    expect(activeTagToken("mytag:x")).toBeNull();
    expect(activeTagToken('tag:"two words" ')).toBeNull();
  });
});

describe("applyTagSuggestion", () => {
  it("replaces the half-typed filter and leaves a space", () => {
    const query = "dragon tag:roy";
    const token = activeTagToken(query);
    expect(token).not.toBeNull();
    if (!token) return;
    expect(applyTagSuggestion(query, token, "royalty")).toBe("dragon tag:royalty ");
    expect(applyTagSuggestion(query, token, "high king")).toBe('dragon tag:"high king" ');
  });

  it("quotes names with spaces only", () => {
    expect(formatTagFilter("a b")).toBe('tag:"a b"');
    expect(formatTagFilter("places/cities")).toBe("tag:places/cities");
  });
});

describe("matchTagNames", () => {
  it("matches anywhere in the name, shortest first, and respects the limit", () => {
    const names = ["places/cities", "places", "royalty", "coast"];
    expect(matchTagNames(names, "plac")).toEqual(["places", "places/cities"]);
    expect(matchTagNames(names, "")).toHaveLength(4);
    expect(matchTagNames(names, "", 2)).toHaveLength(2);
    expect(matchTagNames(names, "zzz")).toEqual([]);
  });
});
