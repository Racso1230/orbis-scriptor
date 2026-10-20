import type {
  GenerateFinished,
  GeneratePhase,
  GenerateProgress,
  GenerationStats,
} from "@/lib/bindings";

/** What the wizard shows while and after a map generation. `jobId` is `null` until the command answered. */
export interface GenerateState {
  status: "idle" | "running" | "finished" | "failed";
  jobId: string | null;
  phase: GeneratePhase;
  done: number;
  total: number;
  /** Filled when the generation succeeded. */
  result: { mapEntityId: string; stats: GenerationStats } | null;
  error: string | null;
  /**
   * Events that arrived before we knew our job id (the backend starts as soon
   * as the command is invoked). They are replayed once the id is known.
   */
  early: (
    { kind: "progress"; event: GenerateProgress } | { kind: "finished"; event: GenerateFinished }
  )[];
}

export const GENERATE_IDLE: GenerateState = {
  status: "idle",
  jobId: null,
  phase: "generating",
  done: 0,
  total: 0,
  result: null,
  error: null,
  early: [],
};

export type GenerateAction =
  | { type: "begin" }
  | { type: "started"; jobId: string }
  | { type: "startFailed"; message: string }
  | { type: "progress"; event: GenerateProgress }
  | { type: "finished"; event: GenerateFinished }
  | { type: "reset" };

function applyProgress(state: GenerateState, event: GenerateProgress): GenerateState {
  return { ...state, phase: event.phase, done: event.done, total: event.total };
}

function applyFinished(state: GenerateState, event: GenerateFinished): GenerateState {
  // `finished` fires exactly once; ignore anything after we are done.
  if (state.status !== "running") return state;
  if (!event.ok || !event.mapEntityId || !event.stats) {
    return { ...state, status: "failed", error: event.error ?? "The map could not be generated." };
  }
  return {
    ...state,
    status: "finished",
    error: null,
    result: { mapEntityId: event.mapEntityId, stats: event.stats },
  };
}

/** Pure state machine for one generation job; events of other jobs are ignored. */
export function generateReducer(state: GenerateState, action: GenerateAction): GenerateState {
  switch (action.type) {
    case "begin":
      return { ...GENERATE_IDLE, status: "running" };
    case "reset":
      return GENERATE_IDLE;
    case "startFailed":
      return { ...state, status: "failed", error: action.message, early: [] };
    case "started": {
      let next: GenerateState = { ...state, jobId: action.jobId, early: [] };
      for (const item of state.early) {
        if (item.event.jobId !== action.jobId) continue;
        next =
          item.kind === "progress"
            ? applyProgress(next, item.event)
            : applyFinished(next, item.event);
      }
      return next;
    }
    case "progress":
      if (state.status !== "running") return state;
      if (state.jobId === null) {
        return { ...state, early: [...state.early, { kind: "progress", event: action.event }] };
      }
      return action.event.jobId === state.jobId ? applyProgress(state, action.event) : state;
    case "finished":
      if (state.status !== "running") return state;
      if (state.jobId === null) {
        return { ...state, early: [...state.early, { kind: "finished", event: action.event }] };
      }
      return action.event.jobId === state.jobId ? applyFinished(state, action.event) : state;
  }
}

/** Share of the whole job each phase covers, as [start, end] of the bar. */
const PHASE_SPAN: Record<GeneratePhase, [number, number]> = {
  generating: [0, 0.5],
  rendering: [0.5, 0.6],
  writing: [0.6, 1],
};

/** Overall progress 0..1 across all phases. */
export function generateFraction(state: GenerateState): number {
  if (state.status === "finished") return 1;
  if (state.status !== "running") return 0;
  const [start, end] = PHASE_SPAN[state.phase];
  const within = state.total > 0 ? Math.min(1, state.done / state.total) : 0;
  return start + (end - start) * within;
}

export const GENERATE_PHASE_LABELS: Record<GeneratePhase, string> = {
  generating: "Shaping land, rivers and biomes…",
  rendering: "Saving the background image…",
  writing: "Writing the map to the world…",
};
