import type { ImportFinished, ImportProgress } from "@/lib/bindings";

/** What the import dialog shows while and after a vault import. `jobId` is `null` until the command answered. */
export interface ImportState {
  status: "idle" | "running" | "done" | "failed";
  jobId: string | null;
  phase: string;
  done: number;
  total: number;
  /** Filled when the import finished successfully. */
  result: Omit<ImportFinished, "jobId" | "ok" | "error"> | null;
  error: string | null;
  /**
   * Events that arrived before we knew our job id. The backend starts working
   * as soon as the command is invoked, so a fast import can finish before the
   * command reply reaches us. They are replayed once the id is known.
   */
  early: (
    { kind: "progress"; event: ImportProgress } | { kind: "finished"; event: ImportFinished }
  )[];
}

export const IMPORT_IDLE: ImportState = {
  status: "idle",
  jobId: null,
  phase: "scanning",
  done: 0,
  total: 0,
  result: null,
  error: null,
  early: [],
};

export type ImportAction =
  | { type: "begin" }
  | { type: "started"; jobId: string }
  | { type: "startFailed"; message: string }
  | { type: "progress"; event: ImportProgress }
  | { type: "finished"; event: ImportFinished }
  | { type: "reset" };

function applyProgress(state: ImportState, event: ImportProgress): ImportState {
  return { ...state, phase: event.phase, done: event.done, total: event.total };
}

function applyFinished(state: ImportState, event: ImportFinished): ImportState {
  // `finished` fires exactly once; ignore anything after we are done.
  if (state.status !== "running") return state;
  if (!event.ok) {
    return { ...state, status: "failed", error: event.error ?? "The import failed." };
  }
  const result = {
    created: event.created,
    skipped: event.skipped,
    images: event.images,
    relationships: event.relationships,
    typesCreated: event.typesCreated,
    warnings: event.warnings,
    moreWarnings: event.moreWarnings,
  };
  return { ...state, status: "done", error: null, result };
}

/** Pure state machine for one import; events of other jobs are ignored. */
export function importReducer(state: ImportState, action: ImportAction): ImportState {
  switch (action.type) {
    case "begin":
      return { ...IMPORT_IDLE, status: "running" };
    case "reset":
      return IMPORT_IDLE;
    case "startFailed":
      return { ...state, status: "failed", error: action.message, early: [] };
    case "started": {
      let next: ImportState = { ...state, jobId: action.jobId, early: [] };
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

/** 0..1, or `null` when the total is not known yet (show an indeterminate bar). */
export function importFraction(state: ImportState): number | null {
  if (state.status === "done") return 1;
  return state.total > 0 ? Math.min(1, state.done / state.total) : null;
}

export const IMPORT_PHASE_LABELS: Record<string, string> = {
  scanning: "Scanning the vault…",
  reading: "Reading notes…",
  converting: "Converting text…",
  writing: "Writing to the world…",
  finishing: "Finishing…",
};
