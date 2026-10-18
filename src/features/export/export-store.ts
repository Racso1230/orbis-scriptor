import { toast } from "sonner";
import { create } from "zustand";
import { commands, events, type MarkdownExportOptions } from "@/lib/bindings";
import { unwrap } from "@/lib/unwrap";
import { IDLE, exportReducer, type ExportAction, type ExportState } from "./export-state";

interface ExportStore {
  state: ExportState;
  dialogOpen: boolean;
  setDialogOpen: (open: boolean) => void;
  /** Opens the dialog; a finished or failed export is cleared, a running one is kept. */
  openDialog: () => void;
  dispatch: (action: ExportAction) => void;
}

export const useExportStore = create<ExportStore>()((set) => ({
  state: IDLE,
  dialogOpen: false,
  setDialogOpen: (dialogOpen) => set({ dialogOpen }),
  openDialog: () =>
    set((s) => ({ dialogOpen: true, state: s.state.status === "running" ? s.state : IDLE })),
  dispatch: (action) => set((s) => ({ state: exportReducer(s.state, action) })),
}));

type Unlisten = () => void;

/**
 * Starts an export. This lives outside any component on purpose: the export
 * keeps running when the dialog is closed, and the listeners must outlive it.
 *
 * Order matters: the listeners are registered BEFORE the command is invoked
 * (the backend may report progress, even finish, before the command returns),
 * and they are removed as soon as the job is finished or failed.
 */
export async function startMarkdownExport(
  targetDir: string,
  options: MarkdownExportOptions,
): Promise<void> {
  const { dispatch } = useExportStore.getState();
  if (useExportStore.getState().state.status === "running") return;

  dispatch({ type: "begin" });
  const unlisteners: Unlisten[] = [];
  let cleaned = false;
  const cleanup = () => {
    if (cleaned) return;
    cleaned = true;
    for (const unlisten of unlisteners) unlisten();
  };
  // Called after every dispatch: when the job has ended, stop listening and tell the user.
  const settle = () => {
    const state = useExportStore.getState().state;
    if (state.status === "done") {
      cleanup();
      toast.success(`Exported ${state.files} files`, { description: state.path ?? undefined });
    } else if (state.status === "failed") {
      cleanup();
      toast.error("Export failed", { description: state.error ?? undefined });
    }
  };
  const send = (action: ExportAction) => {
    dispatch(action);
    settle();
  };

  try {
    unlisteners.push(
      await events.exportProgress.listen((e) => send({ type: "progress", event: e.payload })),
      await events.exportFinished.listen((e) => send({ type: "finished", event: e.payload })),
    );
    const jobId = await unwrap(commands.exportMarkdown(targetDir, options));
    send({ type: "started", jobId });
  } catch (error) {
    cleanup();
    send({
      type: "startFailed",
      message: error instanceof Error ? error.message : String(error),
    });
  }
}
