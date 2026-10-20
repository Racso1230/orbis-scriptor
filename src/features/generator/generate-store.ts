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
  setDialogOpen: (open: boolean) => void;
  /** Opens the wizard; a finished or failed job is cleared, a running one is kept. */
  openDialog: () => void;
  dispatch: (action: GenerateAction) => void;
}

export const useGenerateStore = create<GenerateStore>()((set) => ({
  state: GENERATE_IDLE,
  dialogOpen: false,
  openCount: 0,
  lastParams: null,
  lastScope: null,
  setDialogOpen: (dialogOpen) => set({ dialogOpen }),
  openDialog: () =>
    set((s) => ({
      dialogOpen: true,
      lastParams: s.state.status === "running" ? s.lastParams : null,
      openCount: s.state.status === "running" ? s.openCount : s.openCount + 1,
      state: s.state.status === "running" ? s.state : GENERATE_IDLE,
    })),
  dispatch: (action) => set((s) => ({ state: generateReducer(s.state, action) })),
}));

type Unlisten = () => void;

/**
 * A new map changes the entity list (generated locations show in the sidebar),
 * tags (`generated`), search (settlements are found by name) and the maps.
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
 */
export async function startGeneration(params: GeneratorParams, name: string | null): Promise<void> {
  const { dispatch } = useGenerateStore.getState();
  if (useGenerateStore.getState().state.status === "running") return;

  useGenerateStore.setState({ lastParams: params, lastScope: params.scope });
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
      toast.success("Map generated");
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
    const jobId = await unwrap(commands.generateMap(params, name));
    send({ type: "started", jobId });
  } catch (error) {
    cleanup();
    send({
      type: "startFailed",
      message: error instanceof Error ? error.message : String(error),
    });
  }
}
