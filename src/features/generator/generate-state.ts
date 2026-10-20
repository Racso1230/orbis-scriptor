import type {
  GenerateFinished,
  GeneratePhase,
  GenerateProgress,
  GenerationScope,
  GenerationStats,
} from "@/lib/bindings";

/**
 * Every phase the backend can report. `"placing"` (Phase 14b) is added here
 * as well so this compiles before and after the bindings are regenerated
 * (a union with a duplicate member is the same union).
 */
export type GenerationPhase = GeneratePhase | "placing";

/**
 * Stats including the Phase 14b counters. Optional here so it compiles
 * against older bindings; once regenerated they are plain numbers.
 */
export type PlacesStats = GenerationStats & { settlements?: number; roads?: number };

/** What the wizard shows while and after a map generation. `jobId` is `null` until the command answered. */
export interface GenerateState {
  status: "idle" | "running" | "finished" | "failed";
  jobId: string | null;
  /** What the job generates; decides how the progress bar is split between phases. */
  scope: GenerationScope;
  phase: GenerationPhase;
  done: number;
  total: number;
  /** Filled when the generation succeeded. */
  result: { mapEntityId: string; stats: PlacesStats } | null;
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
  scope: "mapOnly",
  phase: "generating",
  done: 0,
  total: 0,
  result: null,
  error: null,
  early: [],
};

export type GenerateAction =
  | { type: "begin"; scope?: GenerationScope }
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
      return { ...GENERATE_IDLE, status: "running", scope: action.scope ?? "mapOnly" };
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

type Span = [number, number];

/** Share of the whole job each phase covers, as [start, end] of the bar, for a map-only job. */
const MAP_ONLY_SPAN: Record<GenerationPhase, Span> = {
  generating: [0, 0.5],
  // Not sent for map-only jobs; listed so every phase has a span.
  placing: [0.5, 0.5],
  rendering: [0.5, 0.6],
  writing: [0.6, 1],
};

/** The same with places: generating the terrain leaves room for placing settlements and roads. */
const PLACES_SPAN: Record<GenerationPhase, Span> = {
  generating: [0, 0.35],
  placing: [0.35, 0.5],
  rendering: [0.5, 0.6],
  writing: [0.6, 1],
};

/** The bar segment of `phase` for a job of `scope`. */
export function phaseSpan(scope: GenerationScope, phase: GenerationPhase): Span {
  const spans = scope === "mapOnly" ? MAP_ONLY_SPAN : PLACES_SPAN;
  // An unknown phase from a newer backend keeps the bar at its start rather than crashing.
  return spans[phase] ?? [0, 0];
}

/** Overall progress 0..1 across all phases. */
export function generateFraction(state: GenerateState): number {
  if (state.status === "finished") return 1;
  if (state.status !== "running") return 0;
  const [start, end] = phaseSpan(state.scope, state.phase);
  const within = state.total > 0 ? Math.min(1, state.done / state.total) : 0;
  return start + (end - start) * within;
}

export const GENERATE_PHASE_LABELS: Record<GenerationPhase, string> = {
  generating: "Shaping land, rivers and biomes…",
  placing: "Placing settlements and roads…",
  rendering: "Saving the background image…",
  writing: "Writing the map to the world…",
};
