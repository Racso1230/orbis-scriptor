import { describe, expect, it } from "vitest";
import type { EntitySummary, EntityTypeDef } from "@/lib/bindings";
import { buildSidebarRows } from "./sidebar-rows";

const type = (key: string, plural: string): EntityTypeDef => ({
  key,
  label: key,
  plural,
  icon: "",
  color: "#000000",
  builtin: true,
  fields: [],
});
const entity = (id: string, t: string, name: string): EntitySummary => ({
  id,
  type: t,
  name,
  updatedAt: "",
});

const types = [type("character", "Characters"), type("location", "Locations")];
const entities = [
  entity("1", "character", "Ada"),
  entity("2", "character", "Bob"),
  entity("3", "location", "Adria"),
];

describe("buildSidebarRows", () => {
  it("groups entities under type headers with counts", () => {
    const rows = buildSidebarRows(types, entities, "", new Set());
    expect(
      rows.map((r) => (r.kind === "header" ? `H:${r.type.key}:${r.count}` : r.entity.name)),
    ).toEqual(["H:character:2", "Ada", "Bob", "H:location:1", "Adria"]);
  });

  it("hides entities of collapsed groups but keeps the header", () => {
    const rows = buildSidebarRows(types, entities, "", new Set(["character"]));
    expect(rows).toHaveLength(3);
    expect(rows[0]).toMatchObject({ kind: "header", collapsed: true, count: 2 });
  });

  it("filters case-insensitively, drops empty groups and ignores collapse", () => {
    const rows = buildSidebarRows(types, entities, "ad", new Set(["character"]));
    expect(rows.map((r) => (r.kind === "header" ? r.type.key : r.entity.name))).toEqual([
      "character",
      "Ada",
      "location",
      "Adria",
    ]);
  });
});
