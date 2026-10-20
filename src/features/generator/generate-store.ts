import { toast } from "sonner";
import { create } from "zustand";
import { commands, events, type GenerationScope, type GeneratorParams } from "@/lib/bindings";
import { queryClient } from "@/lib/query-client";
import { unwrap } from "@/lib/unwrap";
import {
  GENERATE_IDLE,
  generateReducer,
  type GenerateAction,
  type GenerateState,
} from "./generate-state";

export interface OpenGenerateOptions {
  targetMapEntityId?: string | null;
}

interface GenerateStore {
  state: GenerateState;
  dialogOpen: boolean;
  /** Changes every time the dialog is opened afresh, so the wizard starts with new settings. */
  openCount: number;
  /**
   * Settings of the last generation started from this dialog, so "Generate
   * another" starts from them (a fresh open starts from new random settings).
   */
  lastParams: GeneratorParams | null;
  /**
   * Scope of the last generation. Unlike `lastParams` it survives a fresh
   * open, so someone who generates places keeps getting places by default.
   */
  lastScope: GenerationScope | null;
  /**
   * The existing (empty) map entity the wizard generates into, or `null` for
   * "create a new map". While a job runs it is the running job's target.
   */
  targetMapEntityId: string | null;
  setDialogOpen: (open: boolean) => void;
  /**
   * Opens the wizard; a finished or failed job is cleared, a running one is
   * kept (together with its target). Pass `targetMapEntityId` to generate
   * terrain into that map instead of creating a new one.
   */
  openDialog: (options?: OpenGenerateOptions) => void;
  dispatch: (action: GenerateAction) => void;
}

export const useGenerateStore = create<GenerateStore>()((set) => ({
  state: GENERATE_IDLE,
  dialogOpen: false,
  openCount: 0,
  lastParams: null,
  lastScope: null,
  targetMapEntityId: null,
  setDialogOpen: (dialogOpen) => set({ dialogOpen }),
  openDialog: (options) =>
    set((s) => ({
      dialogOpen: true,
      targetMapEntityId:
        s.state.status === "running" ? s.targetMapEntityId : (options?.targetMapEntityId ?? null),
      lastParams: s.state.status === "running" ? s.lastParams : null,
      openCount: s.state.status === "running" ? s.openCount : s.openCount + 1,
      state: s.state.status === "running" ? s.state : GENERATE_IDLE,
    })),
  dispatch: (action) => set((s) => ({ state: generateReducer(s.state, action) })),
}));

/** Opens the generate wizard, optionally aimed at an existing empty map. */
export function openGenerateWizard(options?: OpenGenerateOptions) {
  useGenerateStore.getState().openDialog(options);
}

type Unlisten = () => void;

/**
 * A new map changes the entity list (generated locations show in the sidebar),
 * tags (`generated`), search (settlements are found by name) and the maps.
 * The `["maps"]` prefix also covers `mapKeys.detail(target)`, so a map that
 * got generated terrain re-renders with its new background and layers.
 */
function invalidateAfterGenerate() {
  return Promise.all(
    [["entities"], ["tags"], ["search"], ["maps"]].map((queryKey) =>
      queryClient.invalidateQueries({ queryKey }),
    ),
  );
}

/**
 * Starts generating a map in the open world. Lives outside any component so
 * the job keeps running (and is reported) when the dialog is closed.
 *
 * The listeners are registered BEFORE the command is invoked (the backend may
 * report progress, even finish, before the command returns) and removed as
 * soon as the job has finished or failed.
 *
 * With `targetMapEntityId` the terrain is written into that existing, empty
 * map (`name` is then ignored by the backend).
 */
export async function startGeneration(
  params: GeneratorParams,
  name: string | null,
  targetMapEntityId: string | null = null,
): Promise<void> {
  const { dispatch } = useGenerateStore.getState();
  if (useGenerateStore.getState().state.status === "running") return;

  useGenerateStore.setState({ lastParams: params, lastScope: params.scope, targetMapEntityId });
  dispatch({ type: "begin", scope: params.scope });
  const unlisteners: Unlisten[] = [];
  let cleaned = false;
  const cleanup = () => {
    if (cleaned) return;
    cleaned = true;
    for (const unlisten of unlisteners) unlisten();
  };
  let announced = false;
  // Called after every dispatch: when the job has ended, stop listening and tell the user.
  const settle = () => {
    const state = useGenerateStore.getState().state;
    if (announced) return;
    if (state.status === "finished") {
      announced = true;
      cleanup();
      void invalidateAfterGenerate();
      toast.success(targetMapEntityId ? "Terrain generated" : "Map generated");
    } else if (state.status === "failed") {
      announced = true;
      cleanup();
      toast.error("Map generation failed", { description: state.error ?? undefined });
    }
  };
  const send = (action: GenerateAction) => {
    dispatch(action);
    settle();
  };

  try {
    unlisteners.push(
      await events.generateProgress.listen((e) => send({ type: "progress", event: e.payload })),
      await events.generateFinished.listen((e) => send({ type: "finished", event: e.payload })),
    );
    const jobId = await unwrap(commands.generateMap(params, name, targetMapEntityId ?? null));
    send({ type: "started", jobId });
  } catch (error) {
    cleanup();
    send({
      type: "startFailed",
      message: error instanceof Error ? error.message : String(error),
    });
  }
}
