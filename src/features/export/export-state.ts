import type { ExportFinished, ExportProgress } from "@/lib/bindings";

/** What the export dialog shows. `jobId` is `null` until the command has answered. */
export interface ExportState {
  status: "idle" | "running" | "done" | "failed";
  jobId: string | null;
  phase: string;
  done: number;
  total: number;
  path: string | null;
  files: number;
  error: string | null;
  /**
   * Events that arrived before we knew our job id. The backend starts working
   * as soon as the command is invoked, so a fast export can finish before the
   * command's reply reaches us. They are replayed once the id is known.
   */
  early: (
    { kind: "progress"; event: ExportProgress } | { kind: "finished"; event: ExportFinished }
  )[];
}

export const IDLE: ExportState = {
  status: "idle",
  jobId: null,
  phase: "preparing",
  done: 0,
  total: 0,
  path: null,
  files: 0,
  error: null,
  early: [],
};

export type ExportAction =
  | { type: "begin" }
  | { type: "started"; jobId: string }
  | { type: "startFailed"; message: string }
  | { type: "progress"; event: ExportProgress }
  | { type: "finished"; event: ExportFinished }
  | { type: "reset" };

function applyProgress(state: ExportState, event: ExportProgress): ExportState {
  return { ...state, phase: event.phase, done: event.done, total: event.total };
}

function applyFinished(state: ExportState, event: ExportFinished): ExportState {
  // `finished` fires exactly once; ignore anything after we are done.
  if (state.status !== "running") return state;
  return event.ok
    ? { ...state, status: "done", path: event.path, files: event.files, error: null }
    : { ...state, status: "failed", error: event.error ?? "The export failed." };
}

/** Pure state machine for one export; events of other jobs are ignored. */
export function exportReducer(state: ExportState, action: ExportAction): ExportState {
  switch (action.type) {
    case "begin":
      return { ...IDLE, status: "running" };
    case "reset":
      return IDLE;
    case "startFailed":
      return { ...state, status: "failed", error: action.message, early: [] };
    case "started": {
      let next: ExportState = { ...state, jobId: action.jobId, early: [] };
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
export function progressFraction(state: ExportState): number | null {
  if (state.status === "done") return 1;
  return state.total > 0 ? Math.min(1, state.done / state.total) : null;
}

export const PHASE_LABELS: Record<string, string> = {
  preparing: "Preparing…",
  writing: "Writing notes…",
  assets: "Copying images…",
  finishing: "Finishing…",
};
