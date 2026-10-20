import { describe, expect, it } from "vitest";
import type { GenerateFinished } from "@/lib/bindings";
import {
  GENERATE_IDLE,
  generateFraction,
  generateReducer,
  GENERATE_PHASE_LABELS,
  type GenerateAction,
  type GenerateState,
  type GenerationPhase,
  type PlacesStats,
} from "./generate-state";

const STATS: PlacesStats = {
  landPercentActual: 41.2,
  landMasses: 3,
  rivers: 12,
  stamps: 340,
  biomeCounts: { ocean: 100 },
  settlements: 24,
  roads: 30,
};
// `as GenerateAction` so "placing" is accepted before the bindings know it.
const progress = (jobId: string, phase: GenerationPhase, done: number, total = 4) =>
  ({ type: "progress", event: { jobId, phase, done, total } }) as GenerateAction;
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

  it("follows the placing phase of a places job", () => {
    const s = run([
      { type: "begin", scope: "places" },
      { type: "started", jobId: "a" },
      progress("a", "placing", 1, 2),
    ]);
    expect(s).toMatchObject({ status: "running", scope: "places", phase: "placing", done: 1 });
    expect(run([{ type: "begin" }]).scope).toBe("mapOnly");
  });

  it("ignores events when not running", () => {
    expect(run([progress("a", "writing", 1)])).toEqual(GENERATE_IDLE);
  });
});

describe("generateFraction", () => {
  it("rises through the phases", () => {
    const at = (phase: GenerationPhase, done: number, total: number) =>
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

  it("makes room for placing in a places job", () => {
    const at = (phase: GenerationPhase, done: number, total: number) =>
      generateFraction(
        run([
          { type: "begin", scope: "places" },
          { type: "started", jobId: "a" },
          progress("a", phase, done, total),
        ]),
      );
    expect(at("generating", 1, 1)).toBeCloseTo(0.35);
    expect(at("placing", 0, 2)).toBeCloseTo(0.35);
    expect(at("placing", 1, 2)).toBeCloseTo(0.425);
    expect(at("placing", 2, 2)).toBeCloseTo(0.5);
    expect(at("rendering", 1, 1)).toBeCloseTo(0.6);
    expect(at("writing", 5, 10)).toBeCloseTo(0.8);
  });
});

describe("GENERATE_PHASE_LABELS", () => {
  it("describes the placing phase", () => {
    expect(GENERATE_PHASE_LABELS.placing).toBe("Placing settlements and roads…");
  });
});
