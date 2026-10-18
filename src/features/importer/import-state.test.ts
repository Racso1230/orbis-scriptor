import { describe, expect, it } from "vitest";
import type { ImportFinished, ImportProgress } from "@/lib/bindings";
import {
  IMPORT_IDLE,
  importFraction,
  importReducer,
  type ImportAction,
  type ImportState,
} from "./import-state";

const progress = (jobId: string, done: number, phase = "reading"): ImportProgress => ({
  jobId,
  done,
  total: 10,
  phase,
});
const finished = (jobId: string, ok: boolean): ImportFinished => ({
  jobId,
  ok,
  created: ok ? 12 : 0,
  skipped: ok ? 2 : 0,
  images: ok ? 3 : 0,
  relationships: ok ? 4 : 0,
  typesCreated: ok ? 1 : 0,
  warnings: ok ? ["Link [[x]] not found"] : [],
  moreWarnings: ok ? 5 : 0,
  error: ok ? null : "The vault changed",
});

function run(actions: ImportAction[], from: ImportState = IMPORT_IDLE) {
  return actions.reduce(importReducer, from);
}

describe("importReducer", () => {
  it("goes from running through progress to a finished result", () => {
    const s = run([
      { type: "begin" },
      { type: "started", jobId: "a" },
      { type: "progress", event: progress("a", 3, "converting") },
      { type: "finished", event: finished("a", true) },
    ]);
    expect(s).toMatchObject({
      status: "done",
      phase: "converting",
      done: 3,
      result: {
        created: 12,
        skipped: 2,
        images: 3,
        relationships: 4,
        typesCreated: 1,
        warnings: ["Link [[x]] not found"],
        moreWarnings: 5,
      },
    });
  });

  it("ignores progress and finish events of other jobs", () => {
    const s = run([
      { type: "begin" },
      { type: "started", jobId: "a" },
      { type: "progress", event: progress("b", 9) },
      { type: "finished", event: finished("b", true) },
    ]);
    expect(s).toMatchObject({ status: "running", done: 0, result: null });
  });

  it("reports ok:false as a failure with the message and no result", () => {
    const s = run([
      { type: "begin" },
      { type: "started", jobId: "a" },
      { type: "finished", event: finished("a", false) },
    ]);
    expect(s).toMatchObject({ status: "failed", error: "The vault changed", result: null });
  });

  it("buffers events that arrive before the job id and replays only its own", () => {
    const s = run([
      { type: "begin" },
      { type: "progress", event: progress("a", 5, "writing") },
      { type: "progress", event: progress("zzz", 9) },
      { type: "finished", event: finished("a", true) },
      { type: "started", jobId: "a" },
    ]);
    expect(s).toMatchObject({ status: "done", phase: "writing", done: 5, early: [] });
  });

  it("ignores events when idle and after the job ended", () => {
    expect(importReducer(IMPORT_IDLE, { type: "progress", event: progress("a", 1) })).toBe(
      IMPORT_IDLE,
    );
    const done = run([
      { type: "begin" },
      { type: "started", jobId: "a" },
      { type: "finished", event: finished("a", true) },
    ]);
    expect(importReducer(done, { type: "finished", event: finished("a", false) })).toBe(done);
    expect(importReducer(done, { type: "progress", event: progress("a", 1) })).toBe(done);
  });

  it("fails when the command itself fails, and can be reset", () => {
    const failed = run([{ type: "begin" }, { type: "startFailed", message: "boom" }]);
    expect(failed).toMatchObject({ status: "failed", error: "boom" });
    expect(importReducer(failed, { type: "reset" })).toEqual(IMPORT_IDLE);
  });
});

describe("importFraction", () => {
  it("is null until a total is known, then done / total", () => {
    expect(importFraction({ ...IMPORT_IDLE, status: "running" })).toBeNull();
    expect(importFraction({ ...IMPORT_IDLE, status: "running", done: 5, total: 10 })).toBe(0.5);
    expect(importFraction({ ...IMPORT_IDLE, status: "done" })).toBe(1);
  });
});
