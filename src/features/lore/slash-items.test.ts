import { describe, expect, it } from "vitest";
import { SLASH_ITEMS, filterSlashItems } from "./slash-items";

describe("filterSlashItems", () => {
  it("returns everything for an empty query", () => {
    expect(filterSlashItems("")).toEqual(SLASH_ITEMS);
    expect(filterSlashItems("   ")).toEqual(SLASH_ITEMS);
  });

  it("matches titles case-insensitively and keyword prefixes", () => {
    expect(filterSlashItems("HEAD").map((i) => i.title)).toEqual([
      "Heading 1",
      "Heading 2",
      "Heading 3",
    ]);
    expect(filterSlashItems("h2").map((i) => i.title)).toEqual(["Heading 2"]);
    expect(filterSlashItems("ul").map((i) => i.title)).toEqual(["Bullet list"]);
  });

  it("returns nothing for junk", () => {
    expect(filterSlashItems("zzz")).toEqual([]);
  });
});
