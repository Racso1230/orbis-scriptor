import { describe, expect, it } from "vitest";
import {
  buildFolderTypeMap,
  choiceToSelectValue,
  customChoiceFor,
  customChoiceProblem,
  firstChoiceProblem,
  type FolderChoice,
} from "./wizard-model";

describe("buildFolderTypeMap", () => {
  it("leaves out folders on the default type", () => {
    expect(buildFolderTypeMap(["Lore", "Misc"], {})).toEqual([]);
    expect(buildFolderTypeMap(["Lore"], { Lore: { mode: "default" } })).toEqual([]);
  });

  it("maps a folder to an existing type", () => {
    const choices: Record<string, FolderChoice> = { Lore: { mode: "type", typeKey: "faction" } };
    expect(buildFolderTypeMap(["Lore", "Misc"], choices)).toEqual([
      { folder: "Lore", typeKey: "faction", createCustom: null },
    ]);
  });

  it("maps a folder to a new custom type (trimmed, lower-case colour)", () => {
    const choices: Record<string, FolderChoice> = {
      Gods: {
        mode: "custom",
        label: " Deity ",
        plural: "Deities",
        icon: "sparkles",
        color: "#AABBCC",
      },
    };
    expect(buildFolderTypeMap(["Gods"], choices)).toEqual([
      {
        folder: "Gods",
        typeKey: null,
        createCustom: { label: "Deity", plural: "Deities", icon: "sparkles", color: "#aabbcc" },
      },
    ]);
  });

  it("ignores choices for folders that are not listed", () => {
    expect(buildFolderTypeMap([], { Gods: { mode: "type", typeKey: "faction" } })).toEqual([]);
  });
});

describe("custom type validation", () => {
  it("starts valid, named after the folder", () => {
    const choice = customChoiceFor("Gods");
    expect(choice).toMatchObject({ label: "Gods", plural: "Gods" });
    expect(customChoiceProblem(choice)).toBeNull();
  });

  it("asks for a name, a plural and a proper colour", () => {
    const base = customChoiceFor("Gods");
    expect(customChoiceProblem({ ...base, label: "  " })).toMatch(/name/);
    expect(customChoiceProblem({ ...base, plural: "" })).toMatch(/plural/);
    expect(customChoiceProblem({ ...base, color: "red" })).toMatch(/colour/);
    expect(customChoiceProblem({ ...base, label: "x".repeat(41) })).toMatch(/at most/);
    expect(customChoiceProblem({ mode: "default" })).toBeNull();
  });

  it("reports the first problem with its folder", () => {
    const choices: Record<string, FolderChoice> = {
      A: customChoiceFor("A"),
      B: { ...customChoiceFor("B"), label: "" },
    };
    expect(firstChoiceProblem(["A", "B"], choices)).toMatch(/^B:/);
    expect(firstChoiceProblem(["A"], choices)).toBeNull();
  });
});

describe("choiceToSelectValue", () => {
  it("names each mode", () => {
    expect(choiceToSelectValue({ mode: "default" })).toBe("default");
    expect(choiceToSelectValue({ mode: "type", typeKey: "faction" })).toBe("type:faction");
    expect(choiceToSelectValue(customChoiceFor("x"))).toBe("custom");
  });
});
