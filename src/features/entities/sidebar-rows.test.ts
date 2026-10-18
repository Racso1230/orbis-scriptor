import { describe, expect, it } from "vitest";
import type { EntitySummary, EntityTypeDef } from "@/lib/bindings";
import { buildSidebarRows, buildTopSections, type SidebarRow } from "./sidebar-rows";

const type = (key: string, plural: string): EntityTypeDef => ({
  key,
  label: key,
  plural,
  icon: "",
  color: "#000000",
  builtin: true,
  fields: [],
});
const entity = (id: string, t: string, name: string, tags: string[] = []): EntitySummary => ({
  id,
  type: t,
  name,
  updatedAt: "",
  favorite: false,
  tags,
  portrait: null,
});

const types = [type("character", "Characters"), type("location", "Locations")];
const entities = [
  entity("1", "character", "Ada", ["royalty"]),
  entity("2", "character", "Bob", ["coast", "places/wilds"]),
  entity("3", "location", "Adria", ["places/cities", "coast"]),
];

/** One short string per row, to compare whole lists at once. */
function describeRow(r: SidebarRow): string {
  if (r.kind === "header") return `H:${r.type.key}:${r.count}`;
  if (r.kind === "section") return `S:${r.id}:${r.count}${r.collapsed ? ":collapsed" : ""}`;
  return `${r.section ? `${r.section}/` : ""}${r.entity.name}`;
}

describe("buildSidebarRows", () => {
  it("groups entities under type headers with counts", () => {
    const rows = buildSidebarRows(types, entities, "", new Set());
    expect(rows.map(describeRow)).toEqual(["H:character:2", "Ada", "Bob", "H:location:1", "Adria"]);
  });

  it("hides entities of collapsed groups but keeps the header", () => {
    const rows = buildSidebarRows(types, entities, "", new Set(["character"]));
    expect(rows).toHaveLength(3);
    expect(rows[0]).toMatchObject({ kind: "header", collapsed: true, count: 2 });
  });

  it("filters case-insensitively, drops empty groups and ignores collapse", () => {
    const rows = buildSidebarRows(types, entities, "ad", new Set(["character"]));
    expect(rows.map(describeRow)).toEqual(["H:character:1", "Ada", "H:location:1", "Adria"]);
  });

  it("filters by tags: any one of them", () => {
    const rows = buildSidebarRows(types, entities, "", new Set(), {
      tags: ["royalty", "places/cities"],
      matchAll: false,
    });
    expect(rows.map(describeRow)).toEqual(["H:character:1", "Ada", "H:location:1", "Adria"]);
  });

  it("filters by tags: all of them", () => {
    const rows = buildSidebarRows(types, entities, "", new Set(), {
      tags: ["coast", "places"],
      matchAll: true,
    });
    // `places` matches the nested tags places/wilds and places/cities.
    expect(rows.map(describeRow)).toEqual(["H:character:1", "Bob", "H:location:1", "Adria"]);
  });

  it("combines the tag filter with the text filter and ignores collapse", () => {
    const rows = buildSidebarRows(types, entities, "b", new Set(["character"]), {
      tags: ["coast"],
      matchAll: false,
    });
    expect(rows.map(describeRow)).toEqual(["H:character:1", "Bob"]);
  });

  it("an empty tag list filters nothing", () => {
    const rows = buildSidebarRows(types, entities, "", new Set(), { tags: [], matchAll: true });
    expect(rows).toHaveLength(5);
  });
});

describe("buildTopSections", () => {
  const fav = [{ ...entity("1", "character", "Ada"), favorite: true }];
  const recent = [entity("3", "location", "Adria"), entity("1", "character", "Ada")];

  it("lists favourites and recently edited entities in their own sections", () => {
    const rows = buildTopSections(fav, recent, new Set(), false);
    expect(rows.map(describeRow)).toEqual([
      "S:favorites:1",
      "favorites/Ada",
      "S:recent:2",
      "recent/Adria",
      "recent/Ada",
    ]);
  });

  it("collapses a section but keeps its header", () => {
    const rows = buildTopSections(fav, recent, new Set(["recent" as const]), false);
    expect(rows.map(describeRow)).toEqual([
      "S:favorites:1",
      "favorites/Ada",
      "S:recent:2:collapsed",
    ]);
  });

  it("leaves out empty sections and everything while filtering", () => {
    expect(buildTopSections([], recent, new Set(), false).map(describeRow)[0]).toBe("S:recent:2");
    expect(buildTopSections(fav, recent, new Set(), true)).toEqual([]);
  });
});
