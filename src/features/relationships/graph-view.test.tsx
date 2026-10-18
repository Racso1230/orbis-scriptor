import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithQuery } from "@/test/render";
import { GraphPage } from "./graph-view";
import { useGraphStore } from "./graph-store";
import { KINDS } from "./test-fixtures";

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const commands = vi.hoisted(() => ({
  listEntities: vi.fn(),
  entityTypes: vi.fn(),
  listRelationshipKinds: vi.fn(),
  getEntityGraph: vi.fn(),
  getFamilyTree: vi.fn(),
}));

vi.mock("@/lib/bindings", () => ({ commands }));
vi.mock("@/features/entities/use-open-entity", () => ({ useOpenEntity: () => vi.fn() }));

// Smoke test only: layout and filter logic are covered by the pure-function tests.
describe("GraphPage", () => {
  beforeAll(() => {
    // React Flow measures its container; jsdom has neither of these.
    class ResizeObserverStub {
      observe() {}
      unobserve() {}
      disconnect() {}
    }
    vi.stubGlobal("ResizeObserver", ResizeObserverStub);
    class DOMMatrixStub {
      m22 = 1;
    }
    vi.stubGlobal("DOMMatrixReadOnly", DOMMatrixStub);
  });

  beforeEach(() => {
    vi.clearAllMocks();
    useGraphStore.setState({ focusId: null, history: [], mode: "graph", depth: 2 });
    commands.entityTypes.mockResolvedValue(
      ok([
        { key: "character", label: "Character", plural: "Characters", icon: "user", fields: [] },
        { key: "faction", label: "Faction", plural: "Factions", icon: "flag", fields: [] },
      ]),
    );
    commands.listRelationshipKinds.mockReturnValue(KINDS);
    commands.listEntities.mockResolvedValue(
      ok([
        { id: "c1", name: "Isolde", type: "character", updatedAt: "" },
        { id: "f1", name: "House Vane", type: "faction", updatedAt: "" },
      ]),
    );
    commands.getEntityGraph.mockResolvedValue(
      ok({
        nodes: [
          { id: "c1", name: "Isolde", type: "character", degree: 0 },
          { id: "f1", name: "House Vane", type: "faction", degree: 1 },
        ],
        edges: [
          {
            id: "e1",
            source: "c1",
            target: "f1",
            kind: "member_of",
            label: "member of",
            symmetric: false,
            category: "political",
          },
        ],
        truncated: false,
      }),
    );
  });

  it("asks for a focus when none is chosen", async () => {
    renderWithQuery(<GraphPage />);
    expect(await screen.findByText(/Choose an entity to see/)).toBeInTheDocument();
    expect(commands.getEntityGraph).not.toHaveBeenCalled();
  });

  it("loads the graph for the focused entity with the chosen depth", async () => {
    useGraphStore.setState({ focusId: "c1" });
    renderWithQuery(<GraphPage />);

    expect(await screen.findByText("House Vane")).toBeInTheDocument();
    expect(commands.getEntityGraph).toHaveBeenCalledWith("c1", 2, {
      kinds: null,
      entityTypes: null,
      maxNodes: null,
    });

    fireEvent.click(screen.getByRole("button", { name: "Depth 3" }));
    await waitFor(() =>
      expect(commands.getEntityGraph).toHaveBeenLastCalledWith("c1", 3, expect.anything()),
    );
  });

  it("explains that the family tree is for characters", async () => {
    useGraphStore.setState({ focusId: "f1", mode: "family" });
    renderWithQuery(<GraphPage />);
    expect(await screen.findByText(/family tree works for characters/i)).toBeInTheDocument();
    expect(commands.getFamilyTree).not.toHaveBeenCalled();
  });

  it("shows a notice when the graph was truncated", async () => {
    commands.getEntityGraph.mockResolvedValue(
      ok({
        nodes: [{ id: "c1", name: "Isolde", type: "character", degree: 0 }],
        edges: [],
        truncated: true,
      }),
    );
    useGraphStore.setState({ focusId: "c1" });
    renderWithQuery(<GraphPage />);
    expect(await screen.findByText(/showing the first 1 entities/)).toBeInTheDocument();
  });
});
