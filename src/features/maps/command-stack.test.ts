import { describe, expect, it } from "vitest";
import {
  canRedo,
  canUndo,
  emptyStack,
  popRedo,
  popUndo,
  pushEntry,
  removeEntry,
  type CommandStack,
} from "./command-stack";

function pushAll(entries: string[], limit?: number): CommandStack<string> {
  return entries.reduce((s, e) => pushEntry(s, e, limit), emptyStack<string>());
}

describe("command stack", () => {
  it("starts empty", () => {
    const s = emptyStack<string>();
    expect(canUndo(s)).toBe(false);
    expect(canRedo(s)).toBe(false);
    expect(popUndo(s).entry).toBeNull();
    expect(popRedo(s).entry).toBeNull();
  });

  it("undoes newest first and redoes in the original order", () => {
    let s = pushAll(["a", "b", "c"]);
    const seen: (string | null)[] = [];
    for (let i = 0; i < 3; i++) {
      const r = popUndo(s);
      s = r.stack;
      seen.push(r.entry);
    }
    expect(seen).toEqual(["c", "b", "a"]);
    expect(canUndo(s)).toBe(false);
    const redone: (string | null)[] = [];
    for (let i = 0; i < 3; i++) {
      const r = popRedo(s);
      s = r.stack;
      redone.push(r.entry);
    }
    expect(redone).toEqual(["a", "b", "c"]);
    expect(canRedo(s)).toBe(false);
  });

  it("truncates the redo branch when a new action follows an undo", () => {
    let s = pushAll(["a", "b", "c"]);
    s = popUndo(popUndo(s).stack).stack; // undo c, b
    expect(canRedo(s)).toBe(true);
    s = pushEntry(s, "d");
    expect(canRedo(s)).toBe(false);
    expect(s.past).toEqual(["a", "d"]);
  });

  it("drops the oldest entries beyond the depth limit", () => {
    const s = pushAll(["a", "b", "c", "d", "e"], 3);
    expect(s.past).toEqual(["c", "d", "e"]);
  });

  it("defaults to a depth of 200", () => {
    const s = pushAll(Array.from({ length: 250 }, (_, i) => String(i)));
    expect(s.past).toHaveLength(200);
    expect(s.past[0]).toBe("50");
  });

  it("removes a rejected entry from either side", () => {
    let s = pushAll(["a", "b", "c"]);
    s = popUndo(s).stack; // future: c
    s = removeEntry(s, (e) => e === "c" || e === "a");
    expect(s).toEqual({ past: ["b"], future: [] });
  });
});
