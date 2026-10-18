import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ImportFinished, ImportOptions, ImportProgress } from "@/lib/bindings";
import { IMPORT_IDLE } from "./import-state";
import { startVaultImport, useImportStore } from "./import-store";

type Handler<T> = (e: { payload: T }) => void;

const h = vi.hoisted(() => ({
  order: [] as string[],
  progress: null as null | ((e: { payload: unknown }) => void),
  finished: null as null | ((e: { payload: unknown }) => void),
  unlistenProgress: vi.fn(),
  unlistenFinished: vi.fn(),
  importVault: vi.fn(),
  invalidate: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock("sonner", () => ({ toast: { success: h.toastSuccess, error: h.toastError } }));
vi.mock("@/lib/query-client", () => ({ queryClient: { invalidateQueries: h.invalidate } }));
vi.mock("@/lib/bindings", () => ({
  commands: { importVault: h.importVault },
  events: {
    importProgress: {
      listen: (fn: Handler<unknown>) => {
        h.order.push("listen-progress");
        h.progress = fn;
        return Promise.resolve(h.unlistenProgress);
      },
    },
    importFinished: {
      listen: (fn: Handler<unknown>) => {
        h.order.push("listen-finished");
        h.finished = fn;
        return Promise.resolve(h.unlistenFinished);
      },
    },
  },
}));

const OPTIONS: ImportOptions = {
  defaultType: "note",
  folderTypeMap: [],
  createMissingLinksAsNotes: false,
  importImages: true,
  skipDuplicates: true,
};
const progress = (jobId: string, done: number): ImportProgress => ({
  jobId,
  done,
  total: 4,
  phase: "writing",
});
const finished = (jobId: string, ok: boolean): ImportFinished => ({
  jobId,
  ok,
  created: 7,
  skipped: 0,
  images: 1,
  relationships: 0,
  typesCreated: 0,
  warnings: [],
  moreWarnings: 0,
  error: ok ? null : "nope",
});
const emit = {
  progress: (p: ImportProgress) => h.progress?.({ payload: p }),
  finished: (p: ImportFinished) => h.finished?.({ payload: p }),
};

describe("startVaultImport", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    h.order.length = 0;
    useImportStore.setState({ state: IMPORT_IDLE, dialogOpen: false, openCount: 0 });
  });

  it("listens before invoking, filters by job id, refreshes data and unlistens on success", async () => {
    h.importVault.mockImplementation(() => {
      h.order.push("invoke");
      return Promise.resolve({ status: "ok", data: "job-1" });
    });
    await startVaultImport("C:/vault", OPTIONS);
    expect(h.order).toEqual(["listen-progress", "listen-finished", "invoke"]);
    expect(h.importVault).toHaveBeenCalledWith("C:/vault", OPTIONS);

    emit.progress(progress("other", 3));
    expect(useImportStore.getState().state.done).toBe(0);
    emit.progress(progress("job-1", 2));
    expect(useImportStore.getState().state).toMatchObject({ status: "running", done: 2 });
    expect(h.unlistenProgress).not.toHaveBeenCalled();
    expect(h.invalidate).not.toHaveBeenCalled();

    emit.finished(finished("job-1", true));
    expect(useImportStore.getState().state).toMatchObject({
      status: "done",
      result: { created: 7 },
    });
    expect(h.unlistenProgress).toHaveBeenCalledTimes(1);
    expect(h.unlistenFinished).toHaveBeenCalledTimes(1);
    expect(h.toastSuccess).toHaveBeenCalledTimes(1);
    const keys = h.invalidate.mock.calls.map((c) => (c[0] as { queryKey: string[] }).queryKey[0]);
    expect(keys).toEqual(expect.arrayContaining(["entities", "tags", "search", "graph"]));
  });

  it("handles a finish that arrives before the command returns", async () => {
    h.importVault.mockImplementation(() => {
      emit.finished(finished("job-2", true));
      return Promise.resolve({ status: "ok", data: "job-2" });
    });
    await startVaultImport("C:/vault", OPTIONS);
    expect(useImportStore.getState().state.status).toBe("done");
    expect(h.toastSuccess).toHaveBeenCalledTimes(1);
    expect(h.unlistenFinished).toHaveBeenCalledTimes(1);
  });

  it("shows ok:false as an error, cleans up and does not refresh", async () => {
    h.importVault.mockResolvedValue({ status: "ok", data: "job-3" });
    await startVaultImport("C:/vault", OPTIONS);
    emit.finished(finished("job-3", false));
    expect(useImportStore.getState().state).toMatchObject({ status: "failed", error: "nope" });
    expect(h.toastError).toHaveBeenCalledTimes(1);
    expect(h.unlistenProgress).toHaveBeenCalledTimes(1);
    expect(h.invalidate).not.toHaveBeenCalled();
  });

  it("fails and unlistens when the command rejects", async () => {
    h.importVault.mockResolvedValue({
      status: "error",
      error: { kind: "validation", message: "another import is running" },
    });
    await startVaultImport("C:/vault", OPTIONS);
    expect(useImportStore.getState().state).toMatchObject({
      status: "failed",
      error: "another import is running",
    });
    expect(h.unlistenProgress).toHaveBeenCalledTimes(1);
  });

  it("keeps running when the dialog is closed, and keeps the wizard when reopened", async () => {
    h.importVault.mockResolvedValue({ status: "ok", data: "job-4" });
    useImportStore.getState().openDialog();
    await startVaultImport("C:/vault", OPTIONS);
    const count = useImportStore.getState().openCount;
    useImportStore.getState().setDialogOpen(false);
    emit.progress(progress("job-4", 1));
    expect(useImportStore.getState().state.status).toBe("running");

    useImportStore.getState().openDialog();
    expect(useImportStore.getState().openCount).toBe(count);
    expect(useImportStore.getState().state.status).toBe("running");

    emit.finished(finished("job-4", true));
    expect(h.toastSuccess).toHaveBeenCalled();
    // A finished import is cleared when the wizard is opened again.
    useImportStore.getState().openDialog();
    expect(useImportStore.getState().state).toEqual(IMPORT_IDLE);
    expect(useImportStore.getState().openCount).toBe(count + 1);
  });

  it("ignores a second start while one is running", async () => {
    h.importVault.mockResolvedValue({ status: "ok", data: "job-5" });
    await startVaultImport("C:/vault", OPTIONS);
    await startVaultImport("C:/vault", OPTIONS);
    expect(h.importVault).toHaveBeenCalledTimes(1);
  });
});
