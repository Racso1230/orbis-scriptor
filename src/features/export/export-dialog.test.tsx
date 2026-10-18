import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { IDLE } from "./export-state";
import { ExportMarkdownDialog } from "./export-dialog";
import { useExportStore } from "./export-store";

const start = vi.hoisted(() => vi.fn());
vi.mock("./export-store", async (orig) => ({
  ...(await orig<typeof import("./export-store")>()),
  startMarkdownExport: start,
}));
vi.mock("./pick-export-folder", () => ({
  pickExportFolder: () => Promise.resolve("C:/out"),
}));

describe("ExportMarkdownDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useExportStore.setState({ state: IDLE, dialogOpen: false });
  });

  it("needs a folder, then starts the export with the chosen options", async () => {
    render(<ExportMarkdownDialog />);
    act(() => useExportStore.getState().openDialog());
    const exportButton = screen.getByRole("button", { name: "Export" });
    expect(exportButton).toBeDisabled();

    fireEvent.click(screen.getByLabelText("Replace an earlier export of this world"));
    fireEvent.click(screen.getByLabelText("Include events"));
    fireEvent.click(screen.getByRole("button", { name: /Choose folder/ }));
    expect(await screen.findByText("C:/out")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Export" }));
    expect(start).toHaveBeenCalledWith("C:/out", {
      includeAssets: true,
      includeMaps: true,
      includeEvents: false,
      overwrite: true,
    });
  });

  it("shows progress, completion and errors from the store", () => {
    render(<ExportMarkdownDialog />);
    act(() => {
      useExportStore.getState().openDialog();
      useExportStore.setState({
        state: { ...IDLE, status: "running", jobId: "a", phase: "writing", done: 5, total: 10 },
      });
    });
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "50");
    expect(screen.getByText(/Writing notes… 5 \/ 10/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Hide" })).toBeInTheDocument();

    act(() =>
      useExportStore.setState({
        state: { ...IDLE, status: "done", path: "C:/out/World", files: 9 },
      }),
    );
    expect(screen.getByRole("status")).toHaveTextContent("Exported 9 files to C:/out/World");

    act(() =>
      useExportStore.setState({ state: { ...IDLE, status: "failed", error: "Folder exists" } }),
    );
    expect(screen.getByRole("alert")).toHaveTextContent("Folder exists");
    expect(screen.getByRole("button", { name: "Try again" })).toBeInTheDocument();
  });
});
