import { describe, expect, it } from "vitest";
import type { EntitySummary, EntityTypeDef } from "@/lib/bindings";
import { buildMentionItems } from "./mention-items";

const types: EntityTypeDef[] = [
  {
    key: "character",
    label: "Character",
    plural: "Characters",
    icon: "",
    color: "#000000",
    builtin: true,
    fields: [],
  },
  {
    key: "location",
    label: "Location",
    plural: "Locations",
    icon: "",
    color: "#000000",
    builtin: true,
    fields: [],
  },
];
const entities: EntitySummary[] = [
  {
    id: "1",
    type: "character",
    name: "Ada",
    updatedAt: "",
    favorite: false,
    tags: [],
    portrait: null,
  },
  {
    id: "2",
    type: "location",
    name: "Adria",
    updatedAt: "",
    favorite: false,
    tags: [],
    portrait: null,
  },
  {
    id: "3",
    type: "character",
    name: "Brad",
    updatedAt: "",
    favorite: false,
    tags: [],
    portrait: null,
  },
];

describe("buildMentionItems", () => {
  it("lists all entities and no create options for an empty query", () => {
    const items = buildMentionItems(entities, types, "");
    expect(items.map((i) => i.title)).toEqual(["Ada", "Adria", "Brad"]);
  });

  it("ranks prefix matches first and offers one create option per type", () => {
    const items = buildMentionItems(entities, types, "ad");
    expect(items.map((i) => i.title)).toEqual([
      "Ada",
      "Adria",
      "Brad",
      "Create character “ad”",
      "Create location “ad”",
    ]);
    expect(items[0]).toMatchObject({ kind: "entity", id: "1", description: "Character" });
    expect(items[3]).toMatchObject({ kind: "create", type: "character", name: "ad" });
  });
});
