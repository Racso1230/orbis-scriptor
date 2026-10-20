import { beforeEach, describe, expect, it, vi } from "vitest";
import type { GenerateFinished, GenerateProgress, GeneratorParams } from "@/lib/bindings";
import { GENERATE_IDLE, type GenerationPhase, type PlacesStats } from "./generate-state";
import { startGeneration, useGenerateStore } from "./generate-store";
import { defaultParams } from "./wizard-model";

type Handler<T> = (e: { payload: T }) => void;

const h = vi.hoisted(() => ({
  order: [] as string[],
  progress: null as null | ((e: { payload: unknown }) => void),
  finished: null as null | ((e: { payload: unknown }) => void),
  unlistenProgress: vi.fn(),
  unlistenFinished: vi.fn(),
  generateMap: vi.fn(),
  invalidate: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock("sonner", () => ({ toast: { success: h.toastSuccess, error: h.toastError } }));
vi.mock("@/lib/query-client", () => ({ queryClient: { invalidateQueries: h.invalidate } }));
vi.mock("@/lib/bindings", () => ({
  commands: { generateMap: h.generateMap },
  events: {
    generateProgress: {
      listen: (fn: Handler<unknown>) => {
        h.order.push("listen-progress");
        h.progress = fn;
        return Promise.resolve(h.unlistenProgress);
      },
    },
    generateFinished: {
      listen: (fn: Handler<unknown>) => {
        h.order.push("listen-finished");
        h.finished = fn;
        return Promise.resolve(h.unlistenFinished);
      },
    },
  },
}));

const PARAMS: GeneratorParams = defaultParams(42);
const progress = (jobId: string, done: number): GenerateProgress => ({
  jobId,
  phase: "writing",
  done,
  total: 4,
});
const STATS: PlacesStats = {
  landPercentActual: 40,
  landMasses: 1,
  rivers: 5,
  stamps: 50,
  biomeCounts: {},
  settlements: 0,
  roads: 0,
};
const finished = (jobId: string, ok: boolean): GenerateFinished => ({
  jobId,
  ok,
  mapEntityId: ok ? "map-1" : null,
  stats: ok ? STATS : null,
  error: ok ? null : "nope",
});
const emit = {
  progress: (p: GenerateProgress) => h.progress?.({ payload: p }),
  finished: (p: GenerateFinished) => h.finished?.({ payload: p }),
};

describe("startGeneration", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    h.order.length = 0;
    useGenerateStore.setState({
      state: GENERATE_IDLE,
      dialogOpen: false,
      openCount: 0,
      lastParams: null,
      lastScope: null,
    });
  });

  it("listens before invoking, follows its job and refreshes data on success", async () => {
    h.generateMap.mockImplementation(() => {
      h.order.push("invoke");
      return Promise.resolve({ status: "ok", data: "job-1" });
    });
    await startGeneration(PARAMS, "Aria");
    expect(h.order).toEqual(["listen-progress", "listen-finished", "invoke"]);
    expect(h.generateMap).toHaveBeenCalledWith(PARAMS, "Aria");
    expect(useGenerateStore.getState().lastParams).toEqual(PARAMS);

    emit.progress(progress("other", 3));
    emit.progress(progress("job-1", 2));
    expect(useGenerateStore.getState().state).toMatchObject({ status: "running", done: 2 });

    emit.finished(finished("job-1", true));
    const state = useGenerateStore.getState().state;
    expect(state.status).toBe("finished");
    expect(state.result?.mapEntityId).toBe("map-1");
    expect(h.unlistenProgress).toHaveBeenCalled();
    expect(h.unlistenFinished).toHaveBeenCalled();
    expect(h.invalidate).toHaveBeenCalledWith({ queryKey: ["entities"] });
    expect(h.invalidate).toHaveBeenCalledWith({ queryKey: ["maps"] });
    expect(h.invalidate).toHaveBeenCalledWith({ queryKey: ["search"] });
    expect(h.toastSuccess).toHaveBeenCalled();
  });

  it("sends a places job as is and follows its placing progress", async () => {
    h.generateMap.mockResolvedValue({ status: "ok", data: "job-4" });
    const params: GeneratorParams = { ...PARAMS, scope: "places", kingdoms: 3 };
    await startGeneration(params, null);
    expect(h.generateMap).toHaveBeenCalledWith(
      expect.objectContaining({ scope: "places", kingdoms: 3 }),
      null,
    );
    expect(useGenerateStore.getState().lastScope).toBe("places");
    // `as GenerateProgress`: "placing" is new in Phase 14b bindings.
    emit.progress({
      jobId: "job-4",
      phase: "placing" as GenerationPhase,
      done: 1,
      total: 2,
    } as GenerateProgress);
    expect(useGenerateStore.getState().state).toMatchObject({
      status: "running",
      scope: "places",
      phase: "placing",
      done: 1,
    });
  });

  it("handles a job that finishes before the command returns", async () => {
    h.generateMap.mockImplementation(() => {
      emit.finished(finished("job-2", true));
      return Promise.resolve({ status: "ok", data: "job-2" });
    });
    await startGeneration(PARAMS, null);
    expect(useGenerateStore.getState().state.status).toBe("finished");
  });

  it("reports a failed job without refreshing", async () => {
    h.generateMap.mockResolvedValue({ status: "ok", data: "job-3" });
    await startGeneration(PARAMS, null);
    emit.finished(finished("job-3", false));
    expect(useGenerateStore.getState().state).toMatchObject({ status: "failed", error: "nope" });
    expect(h.invalidate).not.toHaveBeenCalled();
    expect(h.toastError).toHaveBeenCalled();
  });

  it("reports a command error and stops listening", async () => {
    h.generateMap.mockResolvedValue({
      status: "error",
      error: { kind: "validation", message: "No world is open" },
    });
    await startGeneration(PARAMS, null);
    expect(useGenerateStore.getState().state).toMatchObject({
      status: "failed",
      error: "No world is open",
    });
    expect(h.unlistenProgress).toHaveBeenCalled();
  });

  it("opening the dialog afresh clears a finished job and the last settings", () => {
    useGenerateStore.setState({
      state: { ...GENERATE_IDLE, status: "finished" },
      lastParams: PARAMS,
      lastScope: "places",
    });
    useGenerateStore.getState().openDialog();
    const s = useGenerateStore.getState();
    expect(s.dialogOpen).toBe(true);
    expect(s.state).toEqual(GENERATE_IDLE);
    expect(s.lastParams).toBeNull();
    expect(s.lastScope).toBe("places");
    expect(s.openCount).toBe(1);
  });
});
