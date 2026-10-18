import { describe, expect, it } from "vitest";
import type { FieldKind } from "@/lib/bindings";
import { coerceFieldValue, fieldErrorFor, isUnset } from "./field-values";

const single: FieldKind = { entityRef: { allowedTypes: [], multiple: false } };
const multi: FieldKind = { entityRef: { allowedTypes: ["faction"], multiple: true } };

describe("coerceFieldValue", () => {
  it("treats missing and null as empty for every kind", () => {
    const kinds: FieldKind[] = [
      "text",
      "longText",
      "number",
      "boolean",
      "date",
      "select",
      "multiSelect",
      "url",
      single,
      multi,
    ];
    for (const kind of kinds) {
      expect(coerceFieldValue(kind, undefined)).toBeUndefined();
      expect(coerceFieldValue(kind, null)).toBeUndefined();
    }
  });

  it("text, longText and url accept only non-empty strings", () => {
    for (const kind of ["text", "longText", "url"] as const) {
      expect(coerceFieldValue(kind, "hi")).toBe("hi");
      expect(coerceFieldValue(kind, "")).toBeUndefined();
      expect(coerceFieldValue(kind, 5)).toBeUndefined();
      expect(coerceFieldValue(kind, ["a"])).toBeUndefined();
    }
  });

  it("number accepts finite numbers only", () => {
    expect(coerceFieldValue("number", 0)).toBe(0);
    expect(coerceFieldValue("number", -2.5)).toBe(-2.5);
    expect(coerceFieldValue("number", "12")).toBeUndefined();
    expect(coerceFieldValue("number", Number.NaN)).toBeUndefined();
  });

  it("boolean accepts booleans only", () => {
    expect(coerceFieldValue("boolean", true)).toBe(true);
    expect(coerceFieldValue("boolean", false)).toBe(false);
    expect(coerceFieldValue("boolean", "true")).toBeUndefined();
  });

  it("date accepts integer day numbers only", () => {
    expect(coerceFieldValue("date", -40)).toBe(-40);
    expect(coerceFieldValue("date", 1.5)).toBeUndefined();
    expect(coerceFieldValue("date", "12")).toBeUndefined();
  });

  it("select shows only a value that is still one of the options", () => {
    expect(coerceFieldValue("select", "Sea", ["Sea", "War"])).toBe("Sea");
    expect(coerceFieldValue("select", "Forge", ["Sea", "War"])).toBeUndefined();
    expect(coerceFieldValue("select", 3, ["Sea"])).toBeUndefined();
  });

  it("multiSelect keeps known options in option order and drops the rest", () => {
    const options = ["Sea", "War", "Death"];
    expect(coerceFieldValue("multiSelect", ["Death", "Sea"], options)).toEqual(["Sea", "Death"]);
    expect(coerceFieldValue("multiSelect", ["Gone", "War"], options)).toEqual(["War"]);
    expect(coerceFieldValue("multiSelect", ["Gone"], options)).toBeUndefined();
    expect(coerceFieldValue("multiSelect", [], options)).toBeUndefined();
    expect(coerceFieldValue("multiSelect", "Sea", options)).toBeUndefined();
    expect(coerceFieldValue("multiSelect", ["Sea", 4], options)).toBeUndefined();
  });

  it("entityRef single needs a string, multiple needs a list of strings", () => {
    expect(coerceFieldValue(single, "id-1")).toBe("id-1");
    expect(coerceFieldValue(single, ["id-1"])).toBeUndefined();
    expect(coerceFieldValue(single, "")).toBeUndefined();
    expect(coerceFieldValue(multi, ["a", "b", "a"])).toEqual(["a", "b"]);
    expect(coerceFieldValue(multi, "a")).toBeUndefined();
    expect(coerceFieldValue(multi, [])).toBeUndefined();
    expect(coerceFieldValue(multi, ["a", 1])).toBeUndefined();
  });
});

describe("isUnset", () => {
  it("is true for the empty values that remove a key", () => {
    for (const v of [undefined, null, "", []]) expect(isUnset(v)).toBe(true);
    for (const v of [0, false, "x", ["a"]]) expect(isUnset(v)).toBe(false);
  });
});

describe("fieldErrorFor", () => {
  it("returns the problem only for the matching field label", () => {
    const msg = "Domain: must be one of the options";
    expect(fieldErrorFor({ label: "Domain" }, msg)).toBe("must be one of the options");
    expect(fieldErrorFor({ label: "Symbol" }, msg)).toBeNull();
    expect(fieldErrorFor({ label: "Domain" }, null)).toBeNull();
  });
});
