import { describe, expect, it } from "vitest";
import type { GenerateFinished, GenerateProgress, GenerationStats } from "@/lib/bindings";
import {
  GENERATE_IDLE,
  generateFraction,
  generateReducer,
  type GenerateAction,
  type GenerateState,
} from "./generate-state";

const STATS: GenerationStats = {
  landPercentActual: 41.2,
  landMasses: 3,
  rivers: 12,
  stamps: 340,
  biomeCounts: { ocean: 100 },
};
const progress = (jobId: string, phase: GenerateProgress["phase"], done: number, total = 4) => ({
  type: "progress" as const,
  event: { jobId, phase, done, total },
});
const finished = (jobId: string, ok: boolean): GenerateAction => ({
  type: "finished",
  event: {
    jobId,
    ok,
    mapEntityId: ok ? "map-1" : null,
    stats: ok ? STATS : null,
    error: ok ? null : "disk full",
  } satisfies GenerateFinished,
});
const run = (actions: GenerateAction[], from: GenerateState = GENERATE_IDLE) =>
  actions.reduce(generateReducer, from);

describe("generateReducer", () => {
  it("tracks progress and the result of its own job", () => {
    const s = run([
      { type: "begin" },
      { type: "started", jobId: "a" },
      progress("a", "writing", 2),
      progress("b", "writing", 3),
    ]);
    expect(s).toMatchObject({ status: "running", phase: "writing", done: 2, total: 4 });
    const done = run([finished("b", true), finished("a", true)], s);
    expect(done.status).toBe("finished");
    expect(done.result).toEqual({ mapEntityId: "map-1", stats: STATS });
  });

  it("replays events that arrived before the job id", () => {
    const s = run([
      { type: "begin" },
      progress("a", "generating", 1, 1),
      finished("a", true),
      finished("other", false),
      { type: "started", jobId: "a" },
    ]);
    expect(s.status).toBe("finished");
    expect(s.early).toEqual([]);
  });

  it("reports failures and start errors", () => {
    const failed = run([{ type: "begin" }, { type: "started", jobId: "a" }, finished("a", false)]);
    expect(failed).toMatchObject({ status: "failed", error: "disk full" });
    const notStarted = run([{ type: "begin" }, { type: "startFailed", message: "No world" }]);
    expect(notStarted).toMatchObject({ status: "failed", error: "No world" });
    expect(run([{ type: "reset" }], failed)).toEqual(GENERATE_IDLE);
  });

  it("ignores events when not running", () => {
    expect(run([progress("a", "writing", 1)])).toEqual(GENERATE_IDLE);
  });
});

describe("generateFraction", () => {
  it("rises through the phases", () => {
    const at = (phase: GenerateProgress["phase"], done: number, total: number) =>
      generateFraction(
        run([
          { type: "begin" },
          { type: "started", jobId: "a" },
          progress("a", phase, done, total),
        ]),
      );
    expect(at("generating", 0, 1)).toBe(0);
    expect(at("generating", 1, 1)).toBeCloseTo(0.5);
    expect(at("rendering", 1, 1)).toBeCloseTo(0.6);
    expect(at("writing", 5, 10)).toBeCloseTo(0.8);
    expect(generateFraction(GENERATE_IDLE)).toBe(0);
    expect(generateFraction({ ...GENERATE_IDLE, status: "finished" })).toBe(1);
  });
});
