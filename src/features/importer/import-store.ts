import { toast } from "sonner";
import { create } from "zustand";
import { commands, events, type ImportOptions } from "@/lib/bindings";
import { queryClient } from "@/lib/query-client";
import { unwrap } from "@/lib/unwrap";
import { IMPORT_IDLE, importReducer, type ImportAction, type ImportState } from "./import-state";

interface ImportStore {
  state: ImportState;
  dialogOpen: boolean;
  /** Changes every time the dialog is opened afresh, so the wizard can restart at step one. */
  openCount: number;
  setDialogOpen: (open: boolean) => void;
  /** Opens the wizard; a finished or failed import is cleared, a running one is kept. */
  openDialog: () => void;
  dispatch: (action: ImportAction) => void;
}

export const useImportStore = create<ImportStore>()((set) => ({
  state: IMPORT_IDLE,
  dialogOpen: false,
  openCount: 0,
  setDialogOpen: (dialogOpen) => set({ dialogOpen }),
  openDialog: () =>
    set((s) => ({
      dialogOpen: true,
      openCount: s.state.status === "running" ? s.openCount : s.openCount + 1,
      state: s.state.status === "running" ? s.state : IMPORT_IDLE,
    })),
  dispatch: (action) => set((s) => ({ state: importReducer(s.state, action) })),
}));

type Unlisten = () => void;

/** Everything an import can change: entities, types, tags, search, graphs, maps, galleries. */
function invalidateAfterImport() {
  return Promise.all(
    [["entities"], ["tags"], ["search"], ["graph"], ["relationships"], ["backlinks"], ["gallery"]]
      .concat([["documents"], ["events"], ["maps"]])
      .map((queryKey) => queryClient.invalidateQueries({ queryKey })),
  );
}

/**
 * Starts a vault import. Like the Markdown export this lives outside any
 * component: the import keeps running when the dialog is closed.
 *
 * The listeners are registered BEFORE the command is invoked (the backend may
 * report progress, even finish, before the command returns) and removed as
 * soon as the job is finished or failed.
 */
export async function startVaultImport(vaultDir: string, options: ImportOptions): Promise<void> {
  const { dispatch } = useImportStore.getState();
  if (useImportStore.getState().state.status === "running") return;

  dispatch({ type: "begin" });
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
    const state = useImportStore.getState().state;
    if (announced) return;
    if (state.status === "done") {
      announced = true;
      cleanup();
      void invalidateAfterImport();
      toast.success(`Imported ${state.result?.created ?? 0} notes`, {
        description: state.result?.warnings.length
          ? `${state.result.warnings.length + (state.result.moreWarnings ?? 0)} warnings`
          : undefined,
      });
    } else if (state.status === "failed") {
      announced = true;
      cleanup();
      toast.error("Import failed", { description: state.error ?? undefined });
    }
  };
  const send = (action: ImportAction) => {
    dispatch(action);
    settle();
  };

  try {
    unlisteners.push(
      await events.importProgress.listen((e) => send({ type: "progress", event: e.payload })),
      await events.importFinished.listen((e) => send({ type: "finished", event: e.payload })),
    );
    const jobId = await unwrap(commands.importVault(vaultDir, options));
    send({ type: "started", jobId });
  } catch (error) {
    cleanup();
    send({
      type: "startFailed",
      message: error instanceof Error ? error.message : String(error),
    });
  }
}
