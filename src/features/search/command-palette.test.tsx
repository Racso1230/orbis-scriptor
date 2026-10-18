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
  listTags: vi.fn(),
  listEntities: vi.fn(),
  listFavorites: vi.fn(),
  listRecent: vi.fn(),
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

    commands.entityTypes.mockResolvedValue(
      ok([
        { key: "character", label: "Character", plural: "Characters", icon: "user", fields: [] },
      ]),
    );
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
    commands.listTags.mockResolvedValue(
      ok([
        { id: "t1", name: "royalty", color: null, count: 2 },
        { id: "t2", name: "places/cities", color: null, count: 1 },
        { id: "t3", name: "high king", color: null, count: 1 },
      ]),
    );
    const aldric = {
      id: "e1",
      type: "character",
      name: "Aldric",
      updatedAt: "",
      favorite: true,
      tags: ["royalty"],
      portrait: null,
    };
    commands.listEntities.mockResolvedValue(ok([aldric]));
    commands.listFavorites.mockResolvedValue(ok([aldric]));
    commands.listRecent.mockResolvedValue(ok([{ ...aldric, id: "e2", name: "Brenna" }]));
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

  it("shows favourites and recently edited entities when the query is empty", async () => {
    renderWithQuery(<CommandPalette />);
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    expect(await screen.findByText("Favourites")).toBeInTheDocument();
    expect(await screen.findByText("Recently edited")).toBeInTheDocument();
    fireEvent.click(await screen.findByText("Brenna"));
    expect(openEntity).toHaveBeenCalledWith("e2");
  });

  it("suggests tags after tag: and completes the filter", async () => {
    renderWithQuery(<CommandPalette />);
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    const input = await screen.findByPlaceholderText(/Search entities/);

    fireEvent.change(input, { target: { value: "dragon tag:ro" } });
    fireEvent.click(await screen.findByText("royalty", { selector: "[cmdk-item]" }));
    await waitFor(() => expect(input).toHaveValue("dragon tag:royalty "));

    fireEvent.change(input, { target: { value: "tag:high" } });
    fireEvent.click(await screen.findByText("high king"));
    await waitFor(() => expect(input).toHaveValue('tag:"high king" '));
  });

  it("shows an entity's tags in the results", async () => {
    renderWithQuery(<CommandPalette />);
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    fireEvent.change(await screen.findByPlaceholderText(/Search entities/), {
      target: { value: "king" },
    });
    expect(await screen.findByText("#royalty")).toBeInTheDocument();
  });
});
