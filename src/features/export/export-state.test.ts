import { describe, expect, it } from "vitest";
import type { ExportFinished, ExportProgress } from "@/lib/bindings";
import { IDLE, exportReducer, progressFraction, type ExportState } from "./export-state";

const progress = (jobId: string, done: number, phase = "writing"): ExportProgress => ({
  jobId,
  done,
  total: 10,
  phase,
});
const finished = (jobId: string, ok: boolean): ExportFinished => ({
  jobId,
  ok,
  path: ok ? "C:/out/World" : null,
  files: ok ? 12 : 0,
  error: ok ? null : "Folder exists",
});

function run(actions: Parameters<typeof exportReducer>[1][], from: ExportState = IDLE) {
  return actions.reduce(exportReducer, from);
}

describe("exportReducer", () => {
  it("follows progress of its own job to success", () => {
    const s = run([
      { type: "begin" },
      { type: "started", jobId: "a" },
      { type: "progress", event: progress("a", 3) },
      { type: "finished", event: finished("a", true) },
    ]);
    expect(s).toMatchObject({ status: "done", path: "C:/out/World", files: 12, done: 3 });
  });

  it("ignores events of other jobs", () => {
    const s = run([
      { type: "begin" },
      { type: "started", jobId: "a" },
      { type: "progress", event: progress("b", 9) },
      { type: "finished", event: finished("b", true) },
    ]);
    expect(s).toMatchObject({ status: "running", done: 0 });
  });

  it("reports ok:false as a failure with the message", () => {
    const s = run([
      { type: "begin" },
      { type: "started", jobId: "a" },
      { type: "finished", event: finished("a", false) },
    ]);
    expect(s).toMatchObject({ status: "failed", error: "Folder exists" });
  });

  it("replays events that arrived before the job id was known", () => {
    const s = run([
      { type: "begin" },
      { type: "progress", event: progress("a", 5, "assets") },
      { type: "progress", event: progress("zzz", 9) },
      { type: "finished", event: finished("a", true) },
      { type: "started", jobId: "a" },
    ]);
    expect(s).toMatchObject({ status: "done", phase: "assets", done: 5, early: [] });
  });

  it("ignores events after the job ended", () => {
    const done = run([
      { type: "begin" },
      { type: "started", jobId: "a" },
      { type: "finished", event: finished("a", true) },
    ]);
    expect(exportReducer(done, { type: "finished", event: finished("a", false) })).toBe(done);
    expect(exportReducer(done, { type: "progress", event: progress("a", 1) })).toBe(done);
  });

  it("fails when the command itself fails, and can be reset", () => {
    const failed = run([{ type: "begin" }, { type: "startFailed", message: "boom" }]);
    expect(failed).toMatchObject({ status: "failed", error: "boom" });
    expect(exportReducer(failed, { type: "reset" })).toEqual(IDLE);
  });
});

describe("progressFraction", () => {
  it("is null until a total is known, then done / total", () => {
    expect(progressFraction({ ...IDLE, status: "running" })).toBeNull();
    expect(progressFraction({ ...IDLE, status: "running", done: 5, total: 10 })).toBe(0.5);
    expect(progressFraction({ ...IDLE, status: "done" })).toBe(1);
  });
});
