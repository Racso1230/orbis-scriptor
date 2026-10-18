import { describe, expect, it } from "vitest";
import { endpoints, groupByCategory, kindsForPair, offeredKinds, optionId } from "./kinds";
import { KINDS } from "./test-fixtures";

const keys = (options: ReturnType<typeof offeredKinds>) => options.map(optionId);

describe("offeredKinds", () => {
  it("offers forward and inverse wording for a character", () => {
    const ids = keys(offeredKinds(KINDS, "character"));
    expect(ids).toContain("parent_of:forward");
    expect(ids).toContain("parent_of:inverse"); // "child of"
    expect(ids).toContain("member_of:forward");
    expect(ids).not.toContain("member_of:inverse"); // a character cannot be the faction side
  });

  it("offers only the inverse wording when the type fits the target side only", () => {
    const options = offeredKinds(KINDS, "faction");
    const ids = keys(options);
    expect(ids).toContain("member_of:inverse");
    expect(ids).not.toContain("member_of:forward");
    const hasMember = options.find((o) => o.def.key === "member_of");
    expect(hasMember?.label).toBe("has member");
    expect(hasMember?.otherTypes).toEqual(["character"]);
  });

  it("offers symmetric kinds once", () => {
    const spouse = offeredKinds(KINDS, "character").filter((o) => o.def.key === "spouse_of");
    expect(spouse).toHaveLength(1);
    expect(spouse[0]?.inverse).toBe(false);
  });

  it("hides kinds that do not fit the type at all", () => {
    const ids = keys(offeredKinds(KINDS, "note"));
    expect(ids).not.toContain("parent_of:forward");
    expect(ids).not.toContain("located_in:forward");
    expect(ids).toContain("related_to:forward");
  });

  it("groups by category in display order", () => {
    const groups = groupByCategory(offeredKinds(KINDS, "character").map((o) => o.def));
    expect(groups.map((g) => g.category)).toEqual(["family", "political", "place", "other"]);
  });
});

describe("endpoints", () => {
  it("makes the current entity the source for forward options", () => {
    expect(endpoints({ inverse: false }, "me", "them")).toEqual({
      sourceId: "me",
      targetId: "them",
    });
  });

  it("makes the other entity the source for inverse options", () => {
    expect(endpoints({ inverse: true }, "me", "them")).toEqual({
      sourceId: "them",
      targetId: "me",
    });
  });
});

describe("kindsForPair", () => {
  it("keeps only kinds valid for both endpoint types", () => {
    const keysFor = (s: string, t: string) => kindsForPair(KINDS, s, t).map((k) => k.key);
    expect(keysFor("character", "faction")).toContain("member_of");
    expect(keysFor("character", "character")).not.toContain("member_of");
    expect(keysFor("faction", "character")).not.toContain("member_of");
  });
});
