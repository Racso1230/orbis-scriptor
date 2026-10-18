import { describe, expect, it } from "vitest";
import { matchShortcut, type KeyInput } from "./matcher";

const press = (key: string, mods: Partial<KeyInput> = {}): KeyInput => ({
  key,
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  shiftKey: false,
  ...mods,
});

describe("matchShortcut", () => {
  it("matches Ctrl+letter shortcuts", () => {
    expect(matchShortcut(press("k", { ctrlKey: true }))).toBe("palette");
    expect(matchShortcut(press("n", { ctrlKey: true }))).toBe("newEntity");
    expect(matchShortcut(press("w", { ctrlKey: true }))).toBe("closeTab");
    expect(matchShortcut(press("s", { ctrlKey: true }))).toBe("save");
  });

  it("accepts Cmd (metaKey) as the modifier too", () => {
    expect(matchShortcut(press("s", { metaKey: true }))).toBe("save");
  });

  it("is case-insensitive (Caps Lock)", () => {
    expect(matchShortcut(press("S", { ctrlKey: true }))).toBe("save");
  });

  it("ignores keys without the modifier", () => {
    expect(matchShortcut(press("s"))).toBeNull();
    expect(matchShortcut(press("n"))).toBeNull();
  });

  it("ignores Alt and Shift variants", () => {
    expect(matchShortcut(press("s", { ctrlKey: true, shiftKey: true }))).toBeNull();
    expect(matchShortcut(press("w", { ctrlKey: true, altKey: true }))).toBeNull();
  });

  it("matches undo and redo (Ctrl+Z, Ctrl+Shift+Z, Ctrl+Y)", () => {
    expect(matchShortcut(press("z", { ctrlKey: true }))).toBe("undo");
    expect(matchShortcut(press("Z", { ctrlKey: true, shiftKey: true }))).toBe("redo");
    expect(matchShortcut(press("y", { ctrlKey: true }))).toBe("redo");
    expect(matchShortcut(press("z", { metaKey: true }))).toBe("undo");
    expect(matchShortcut(press("y", { ctrlKey: true, shiftKey: true }))).toBeNull();
    expect(matchShortcut(press("z", { ctrlKey: true, altKey: true }))).toBeNull();
  });

  it("ignores unrelated shortcuts", () => {
    expect(matchShortcut(press("b", { ctrlKey: true }))).toBeNull();
    expect(matchShortcut(press("Control", { ctrlKey: true }))).toBeNull();
  });
});
