import { describe, expect, it } from "vitest";
import type { EntityTypeDef } from "@/lib/bindings";
import {
  describeRisks,
  draftFromType,
  draftSignature,
  moveItem,
  newFieldDraft,
  newTypeDraft,
  reassignTargets,
  suggestPlural,
  toFieldInputs,
  withLabel,
  withPlural,
} from "./draft";

const deity: EntityTypeDef = {
  key: "custom-deity",
  label: "Deity",
  plural: "Deities",
  icon: "sparkles",
  color: "#a855f7",
  builtin: false,
  fields: [
    { key: "domain", label: "Domain", kind: "select", options: ["Sea", "War"], custom: true },
    {
      key: "worshippers",
      label: "Worshippers",
      kind: { entityRef: { allowedTypes: ["faction"], multiple: true } },
      options: [],
      custom: true,
    },
  ],
};

describe("plural suggestion", () => {
  it("follows the label until the plural is edited by hand", () => {
    let d = newTypeDraft();
    d = withLabel(d, "Spell");
    expect(d.plural).toBe("Spells");
    d = withPlural(d, "Spells of power");
    d = withLabel(d, "Charm");
    expect(d.plural).toBe("Spells of power");
    expect(suggestPlural("  ")).toBe("");
  });

  it("never rewrites the plural of an existing type", () => {
    expect(withLabel(draftFromType(deity), "Gods").plural).toBe("Deities");
  });
});

describe("toFieldInputs", () => {
  it("keeps keys of existing fields and sends null for new ones", () => {
    const draft = draftFromType(deity);
    const added = { ...newFieldDraft(), label: "  Symbol ", kind: "text" as const };
    const inputs = toFieldInputs([...draft.fields, added]);
    expect(inputs.map((f) => f.key)).toEqual(["domain", "worshippers", null]);
    expect(inputs[2]?.label).toBe("Symbol");
    expect(inputs[1]?.kind).toEqual({ entityRef: { allowedTypes: ["faction"], multiple: true } });
  });

  it("only sends options for select kinds and drops blank ones", () => {
    const f = { ...newFieldDraft(), label: "A", kind: "multiSelect" as const, options: ["x", " "] };
    expect(toFieldInputs([f])[0]?.options).toEqual(["x"]);
    expect(toFieldInputs([{ ...f, kind: "text" }])[0]?.options).toEqual([]);
  });

  it("keeps the key when a field is reordered", () => {
    const draft = draftFromType(deity);
    const moved = moveItem(draft.fields, 0, 1);
    expect(toFieldInputs(moved).map((f) => f.key)).toEqual(["worshippers", "domain"]);
  });
});

describe("built-in types", () => {
  it("only expose their user-added fields", () => {
    const character: EntityTypeDef = {
      key: "character",
      label: "Character",
      plural: "Characters",
      icon: "user",
      color: "#2563eb",
      builtin: true,
      fields: [
        { key: "age", label: "Age", kind: "number", options: [], custom: false },
        { key: "eye", label: "Eye colour", kind: "text", options: [], custom: true },
      ],
    };
    expect(draftFromType(character).fields.map((f) => f.key)).toEqual(["eye"]);
  });
});

describe("describeRisks and dirty state", () => {
  it("warns about removed fields, kind changes and lost options", () => {
    const initial = draftFromType(deity);
    const draft = draftFromType(deity);
    draft.fields = draft.fields.slice(1).map((f) => ({ ...f }));
    expect(describeRisks(initial, draft).join(" ")).toContain("Removing “Domain”");

    const second = { ...initial, fields: initial.fields.map((f) => ({ ...f })) };
    second.fields[0] = { ...second.fields[0]!, kind: "text" };
    expect(describeRisks(initial, second).join(" ")).toContain("Changing “Domain”");

    const third = { ...initial, fields: initial.fields.map((f) => ({ ...f })) };
    third.fields[0] = { ...third.fields[0]!, options: ["Sea", "Storm"] };
    expect(describeRisks(initial, third).join(" ")).toContain("Renaming or removing options");
  });

  it("signature ignores local ids but notices edits", () => {
    expect(draftSignature(draftFromType(deity))).toBe(draftSignature(draftFromType(deity)));
    expect(draftSignature(withLabel(draftFromType(deity), "God"))).not.toBe(
      draftSignature(draftFromType(deity)),
    );
  });
});

describe("reassignTargets", () => {
  it("excludes the deleted type, map and event", () => {
    const types = ["character", "map", "event", "custom-deity", "custom-spell"].map((key) => ({
      ...deity,
      key,
    }));
    expect(reassignTargets(types, "custom-deity").map((t) => t.key)).toEqual([
      "character",
      "custom-spell",
    ]);
  });
});
