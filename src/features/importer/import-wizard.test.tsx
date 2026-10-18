import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ImportPreview } from "@/lib/bindings";
import { renderWithQuery } from "@/test/render";
import { IMPORT_IDLE } from "./import-state";
import { ImportWizard } from "./import-wizard";
import { useImportStore } from "./import-store";

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const commands = vi.hoisted(() => ({
  entityTypes: vi.fn(),
  listTypeIcons: vi.fn(),
  previewImport: vi.fn(),
}));
const start = vi.hoisted(() => vi.fn());
vi.mock("@/lib/bindings", () => ({ commands, events: {} }));
vi.mock("./import-store", async (orig) => ({
  ...(await orig<typeof import("./import-store")>()),
  startVaultImport: start,
}));
vi.mock("./pick-vault-folder", () => ({
  pickVaultFolder: () => Promise.resolve("C:/vault"),
}));

const PREVIEW: ImportPreview = {
  notes: 5,
  byType: [
    { typeKey: "character", label: "Character", count: 3, newType: false },
    { typeKey: "custom-deity", label: "Deity", count: 2, newType: true },
  ],
  unresolvedTypes: [
    { folder: "Gods", count: 2 },
    { folder: "Misc", count: 1 },
  ],
  images: 2,
  tags: 4,
  skipped: 0,
  relationships: 0,
  orbisExport: false,
  warnings: ["Link [[Nobody]] has no note"],
  moreWarnings: 3,
};

const type = (key: string, label: string) => ({
  key,
  label,
  plural: `${label}s`,
  icon: "user",
  color: "#2563eb",
  builtin: true,
  fields: [],
});

async function openAtPreview() {
  renderWithQuery(<ImportWizard />);
  act(() => useImportStore.getState().openDialog());
  fireEvent.click(await screen.findByRole("button", { name: /Choose vault folder/ }));
  expect(await screen.findByText("C:/vault")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Next" }));
  fireEvent.click(screen.getByRole("button", { name: "Next" }));
  await screen.findByRole("region", { name: "Folder mapping" });
}

describe("ImportWizard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useImportStore.setState({ state: IMPORT_IDLE, dialogOpen: false, openCount: 0 });
    commands.entityTypes.mockResolvedValue(
      ok([type("character", "Character"), type("faction", "Faction"), type("note", "Note")]),
    );
    commands.listTypeIcons.mockResolvedValue(["user", "box", "sparkles"]);
    commands.previewImport.mockResolvedValue(ok(PREVIEW));
  });

  it("needs a folder before it can continue", async () => {
    renderWithQuery(<ImportWizard />);
    act(() => useImportStore.getState().openDialog());
    expect(await screen.findByRole("button", { name: "Next" })).toBeDisabled();
  });

  it("shows the preview: counts by type, new types, stats and warnings", async () => {
    await openAtPreview();
    expect(screen.getByRole("status")).toHaveTextContent("5 notes will be imported");
    expect(screen.getByText("Deity: 2 (new type)")).toBeInTheDocument();
    expect(screen.getByText("Character: 3")).toBeInTheDocument();
    expect(screen.getByText("Link [[Nobody]] has no note")).toBeInTheDocument();
    expect(screen.getByText("and 3 more")).toBeInTheDocument();
    expect(commands.previewImport).toHaveBeenCalledWith(
      "C:/vault",
      expect.objectContaining({ defaultType: "note", folderTypeMap: [], skipDuplicates: true }),
    );
  });

  it("maps folders to an existing type or a new custom type and starts the import", async () => {
    await openAtPreview();
    fireEvent.change(screen.getByLabelText("Type for folder Gods"), {
      target: { value: "custom" },
    });
    fireEvent.change(screen.getByLabelText("Name (singular)"), { target: { value: "Deity" } });
    fireEvent.change(screen.getByLabelText("Plural"), { target: { value: "Deities" } });
    fireEvent.change(screen.getByLabelText("Type for folder Misc"), {
      target: { value: "type:faction" },
    });

    fireEvent.click(screen.getByRole("button", { name: "Import" }));
    await waitFor(() => expect(start).toHaveBeenCalled());
    const [dir, options] = start.mock.calls[0]!;
    expect(dir).toBe("C:/vault");
    expect(options.folderTypeMap).toEqual([
      {
        folder: "Gods",
        typeKey: null,
        createCustom: { label: "Deity", plural: "Deities", icon: "box", color: "#0891b2" },
      },
      { folder: "Misc", typeKey: "faction", createCustom: null },
    ]);
  });

  it("refreshes the preview when a mapping changes", async () => {
    await openAtPreview();
    fireEvent.change(screen.getByLabelText("Type for folder Misc"), {
      target: { value: "type:faction" },
    });
    await waitFor(() =>
      expect(commands.previewImport).toHaveBeenCalledWith(
        "C:/vault",
        expect.objectContaining({
          folderTypeMap: [{ folder: "Misc", typeKey: "faction", createCustom: null }],
        }),
      ),
    );
  });

  it("blocks the import while a custom type form is invalid", async () => {
    await openAtPreview();
    fireEvent.change(screen.getByLabelText("Type for folder Gods"), {
      target: { value: "custom" },
    });
    fireEvent.change(screen.getByLabelText("Name (singular)"), { target: { value: "  " } });
    expect(screen.getByRole("alert")).toHaveTextContent(/Give the new type a name/);
    expect(screen.getByRole("button", { name: "Import" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Import" }));
    expect(start).not.toHaveBeenCalled();
  });

  it("uses the options chosen on the options step", async () => {
    renderWithQuery(<ImportWizard />);
    act(() => useImportStore.getState().openDialog());
    fireEvent.click(await screen.findByRole("button", { name: /Choose vault folder/ }));
    await screen.findByText("C:/vault");
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    fireEvent.change(await screen.findByLabelText("Type for notes without a clear type"), {
      target: { value: "faction" },
    });
    fireEvent.click(screen.getByLabelText(/Skip notes that already exist/));
    fireEvent.click(screen.getByLabelText(/Turn links to missing notes/));
    fireEvent.click(screen.getByLabelText(/Import images/));
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    await screen.findByRole("region", { name: "Folder mapping" });
    fireEvent.click(screen.getByRole("button", { name: "Import" }));
    await waitFor(() => expect(start).toHaveBeenCalled());
    expect(start.mock.calls[0]![1]).toMatchObject({
      defaultType: "faction",
      skipDuplicates: false,
      createMissingLinksAsNotes: true,
      importImages: false,
    });
  });

  it("shows progress while running and the summary with warnings when done", async () => {
    renderWithQuery(<ImportWizard />);
    act(() => {
      useImportStore.getState().openDialog();
      useImportStore.setState({
        state: {
          ...IMPORT_IDLE,
          status: "running",
          jobId: "j",
          phase: "writing",
          done: 5,
          total: 10,
        },
      });
    });
    expect(await screen.findByRole("progressbar")).toHaveAttribute("aria-valuenow", "50");
    expect(screen.getByText(/Writing to the world… 5 \/ 10/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Hide" })).toBeInTheDocument();

    act(() =>
      useImportStore.setState({
        state: {
          ...IMPORT_IDLE,
          status: "done",
          result: {
            created: 9,
            skipped: 1,
            images: 2,
            relationships: 3,
            typesCreated: 1,
            warnings: ["odd link"],
            moreWarnings: 4,
          },
        },
      }),
    );
    expect(screen.getByRole("status")).toHaveTextContent("Imported 9 entities, skipped 1.");
    expect(screen.getByText("odd link")).toBeInTheDocument();
    expect(screen.getByText("and 4 more")).toBeInTheDocument();

    act(() =>
      useImportStore.setState({ state: { ...IMPORT_IDLE, status: "failed", error: "Disk full." } }),
    );
    expect(screen.getByRole("alert")).toHaveTextContent("Disk full. Nothing was imported.");
    expect(screen.getByRole("button", { name: "Start over" })).toBeInTheDocument();
  });
});
