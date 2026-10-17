import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithQuery } from "@/test/render";
import { WorldEmptyState } from "./world-layout";

const commands = vi.hoisted(() => ({
  entityTypes: vi.fn(),
  listEntities: vi.fn(),
  createEntity: vi.fn(),
}));
const openEntity = vi.hoisted(() => vi.fn());

vi.mock("@/lib/bindings", () => ({ commands }));
vi.mock("@/features/entities/use-open-entity", () => ({ useOpenEntity: () => openEntity }));
// world-layout imports components that need the router; only WorldEmptyState is under test.
vi.mock("@tanstack/react-router", () => ({ Outlet: () => null, useRouterState: () => "/" }));
vi.mock("@/features/search/command-palette", () => ({ CommandPalette: () => null }));
vi.mock("@/features/entities/entity-sidebar", () => ({ EntitySidebar: () => null }));
vi.mock("@/features/entities/tab-bar", () => ({ TabBar: () => null }));
vi.mock("@/features/shortcuts/shortcuts-host", () => ({ ShortcutsHost: () => null }));

const types = ["character", "location", "map", "event"].map((key) => ({
  key,
  label: key.charAt(0).toUpperCase() + key.slice(1),
  plural: key + "s",
  icon: "x",
  fields: [],
}));

describe("WorldEmptyState", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    commands.entityTypes.mockResolvedValue(types);
    commands.createEntity.mockResolvedValue({
      status: "ok",
      data: { id: "new-1", type: "character", name: "New Character", summary: "", properties: {} },
    });
  });

  it("offers quick actions in a world with no entities and creates on click", async () => {
    commands.listEntities.mockResolvedValue({ status: "ok", data: [] });
    renderWithQuery(<WorldEmptyState />);

    expect(await screen.findByText("Your world is empty")).toBeInTheDocument();
    expect(screen.getByText(/to search or run commands/)).toBeInTheDocument();
    for (const label of ["character", "location", "map", "event"]) {
      expect(screen.getByRole("button", { name: `New ${label}` })).toBeInTheDocument();
    }

    const button = screen.getByRole("button", { name: "New character" });
    await waitFor(() => expect(button).toBeEnabled());
    fireEvent.click(button);
    await waitFor(() =>
      expect(commands.createEntity).toHaveBeenCalledWith({
        type: "character",
        name: "New Character",
      }),
    );
    await waitFor(() => expect(openEntity).toHaveBeenCalledWith("new-1"));
  });

  it("shows the plain hint when the world already has entities", async () => {
    commands.listEntities.mockResolvedValue({
      status: "ok",
      data: [{ id: "a", type: "character", name: "Aldric", summary: "" }],
    });
    renderWithQuery(<WorldEmptyState />);
    expect(await screen.findByText(/Pick an entity in the sidebar/)).toBeInTheDocument();
    expect(screen.queryByText("Your world is empty")).not.toBeInTheDocument();
  });
});
