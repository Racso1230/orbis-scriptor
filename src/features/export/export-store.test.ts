import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ExportFinished, ExportProgress } from "@/lib/bindings";
import { IDLE } from "./export-state";
import { startMarkdownExport, useExportStore } from "./export-store";

type Handler<T> = (e: { payload: T }) => void;

const h = vi.hoisted(() => ({
  order: [] as string[],
  progress: null as null | ((e: { payload: unknown }) => void),
  finished: null as null | ((e: { payload: unknown }) => void),
  unlistenProgress: vi.fn(),
  unlistenFinished: vi.fn(),
  exportMarkdown: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock("sonner", () => ({ toast: { success: h.toastSuccess, error: h.toastError } }));
vi.mock("@/lib/bindings", () => ({
  commands: { exportMarkdown: h.exportMarkdown },
  events: {
    exportProgress: {
      listen: (fn: Handler<unknown>) => {
        h.order.push("listen-progress");
        h.progress = fn;
        return Promise.resolve(h.unlistenProgress);
      },
    },
    exportFinished: {
      listen: (fn: Handler<unknown>) => {
        h.order.push("listen-finished");
        h.finished = fn;
        return Promise.resolve(h.unlistenFinished);
      },
    },
  },
}));

const OPTIONS = { includeAssets: true, includeMaps: true, includeEvents: true, overwrite: false };
const progress = (jobId: string, done: number): ExportProgress => ({
  jobId,
  done,
  total: 4,
  phase: "writing",
});
const finished = (jobId: string, ok: boolean): ExportFinished => ({
  jobId,
  ok,
  path: ok ? "C:/out/W" : null,
  files: 7,
  error: ok ? null : "nope",
});
const emit = {
  progress: (p: ExportProgress) => h.progress?.({ payload: p }),
  finished: (p: ExportFinished) => h.finished?.({ payload: p }),
};

describe("startMarkdownExport", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    h.order.length = 0;
    useExportStore.setState({ state: IDLE, dialogOpen: false });
  });

  it("listens before invoking, filters by job id, toasts and unlistens on success", async () => {
    h.exportMarkdown.mockImplementation(() => {
      h.order.push("invoke");
      return Promise.resolve({ status: "ok", data: "job-1" });
    });
    await startMarkdownExport("C:/out", OPTIONS);
    expect(h.order).toEqual(["listen-progress", "listen-finished", "invoke"]);
    expect(h.exportMarkdown).toHaveBeenCalledWith("C:/out", OPTIONS);

    emit.progress(progress("other", 3));
    expect(useExportStore.getState().state.done).toBe(0);
    emit.progress(progress("job-1", 2));
    expect(useExportStore.getState().state).toMatchObject({ status: "running", done: 2, total: 4 });
    expect(h.unlistenProgress).not.toHaveBeenCalled();

    emit.finished(finished("job-1", true));
    expect(useExportStore.getState().state).toMatchObject({ status: "done", files: 7 });
    expect(h.unlistenProgress).toHaveBeenCalledTimes(1);
    expect(h.unlistenFinished).toHaveBeenCalledTimes(1);
    expect(h.toastSuccess).toHaveBeenCalledWith(
      "Exported 7 files",
      expect.objectContaining({ description: "C:/out/W" }),
    );
  });

  it("handles a finish that arrives before the command returns", async () => {
    h.exportMarkdown.mockImplementation(() => {
      emit.finished(finished("job-2", true));
      return Promise.resolve({ status: "ok", data: "job-2" });
    });
    await startMarkdownExport("C:/out", OPTIONS);
    expect(useExportStore.getState().state.status).toBe("done");
    expect(h.toastSuccess).toHaveBeenCalledTimes(1);
    expect(h.unlistenFinished).toHaveBeenCalledTimes(1);
  });

  it("shows ok:false as an error and cleans up", async () => {
    h.exportMarkdown.mockResolvedValue({ status: "ok", data: "job-3" });
    await startMarkdownExport("C:/out", OPTIONS);
    emit.finished(finished("job-3", false));
    expect(useExportStore.getState().state).toMatchObject({ status: "failed", error: "nope" });
    expect(h.toastError).toHaveBeenCalledTimes(1);
    expect(h.unlistenProgress).toHaveBeenCalledTimes(1);
  });

  it("fails and unlistens when the command rejects", async () => {
    h.exportMarkdown.mockResolvedValue({
      status: "error",
      error: { kind: "validation", message: "no world" },
    });
    await startMarkdownExport("C:/out", OPTIONS);
    expect(useExportStore.getState().state).toMatchObject({ status: "failed", error: "no world" });
    expect(h.unlistenProgress).toHaveBeenCalledTimes(1);
  });

  it("keeps running when the dialog is closed", async () => {
    h.exportMarkdown.mockResolvedValue({ status: "ok", data: "job-4" });
    useExportStore.getState().openDialog();
    await startMarkdownExport("C:/out", OPTIONS);
    useExportStore.getState().setDialogOpen(false);
    emit.progress(progress("job-4", 1));
    expect(useExportStore.getState().state.status).toBe("running");
    emit.finished(finished("job-4", true));
    expect(h.toastSuccess).toHaveBeenCalled();
  });

  it("ignores a second start while one is running", async () => {
    h.exportMarkdown.mockResolvedValue({ status: "ok", data: "job-5" });
    await startMarkdownExport("C:/out", OPTIONS);
    await startMarkdownExport("C:/out", OPTIONS);
    expect(h.exportMarkdown).toHaveBeenCalledTimes(1);
  });
});
