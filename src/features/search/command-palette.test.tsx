import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithQuery } from "@/test/render";
import { CommandPalette } from "./command-palette";

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const commands = vi.hoisted(() => ({
  entityTypes: vi.fn(),
  createEntity: vi.fn(),
  closeWorld: vi.fn(),
  search: vi.fn(),
  rebuildSearchIndex: vi.fn(),
  devSeedEntities: vi.fn(),
}));
const openEntity = vi.hoisted(() => vi.fn());

vi.mock("@/lib/bindings", () => ({ commands }));
vi.mock("@/features/entities/use-open-entity", () => ({ useOpenEntity: () => openEntity }));
vi.mock("@tanstack/react-router", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@tanstack/react-router")>()),
  useNavigate: () => vi.fn().mockResolvedValue(undefined),
}));

describe("CommandPalette", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // jsdom lacks these browser APIs, which cmdk uses.
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    };
    Element.prototype.scrollIntoView = vi.fn();

    commands.entityTypes.mockResolvedValue([
      { key: "character", label: "Character", plural: "Characters", icon: "user", fields: [] },
    ]);
    commands.search.mockResolvedValue(
      ok([
        {
          entityId: "e1",
          entityType: "character",
          name: "Aldric",
          snippet: "the \u0002king\u0003 of the north",
        },
      ]),
    );
    commands.rebuildSearchIndex.mockResolvedValue(ok(null));
  });

  it("opens on Ctrl+K, shows highlighted results and opens the selection", async () => {
    renderWithQuery(<CommandPalette />);
    expect(screen.queryByPlaceholderText(/Search entities/)).not.toBeInTheDocument();

    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    const input = await screen.findByPlaceholderText(/Search entities/);

    fireEvent.change(input, { target: { value: "king" } });

    const mark = await screen.findByText("king", { selector: "mark" });
    expect(mark).toBeInTheDocument();
    expect(commands.search).toHaveBeenCalledWith("king", 20);

    fireEvent.click(screen.getByText("Aldric"));
    expect(openEntity).toHaveBeenCalledWith("e1");
  });

  it("shows the success message after rebuilding the index", async () => {
    renderWithQuery(<CommandPalette />);
    fireEvent.keyDown(window, { key: "k", metaKey: true });
    await screen.findByPlaceholderText(/Search entities/);

    fireEvent.click(await screen.findByText("Rebuild search index"));

    await waitFor(() => expect(commands.rebuildSearchIndex).toHaveBeenCalled());
    expect(await screen.findByText("Search index rebuilt.")).toBeInTheDocument();
  });
});
