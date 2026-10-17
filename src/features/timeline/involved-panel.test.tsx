import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithQuery } from "@/test/render";
import { InvolvedPanel } from "./involved-panel";

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const commands = vi.hoisted(() => ({
  listInvolved: vi.fn(),
  addInvolved: vi.fn(),
  removeInvolved: vi.fn(),
  entityTypes: vi.fn(),
  createEntity: vi.fn(),
}));
const openEntity = vi.hoisted(() => vi.fn());

vi.mock("@/lib/bindings", () => ({ commands }));
vi.mock("@/features/entities/use-open-entity", () => ({ useOpenEntity: () => openEntity }));
// The real picker searches the entity list; here it just offers one fixed choice.
vi.mock("@/features/maps/entity-picker-dialog", () => ({
  EntityPickerDialog: ({
    open,
    onPickEntity,
  }: {
    open: boolean;
    onPickEntity: (id: string) => void;
  }) =>
    open ? (
      <>
        <button onClick={() => onPickEntity("c3")}>Pick Mira</button>
        <button onClick={() => onPickEntity("c1")}>Pick Aldric</button>
      </>
    ) : null,
}));

const summary = (id: string, name: string, type = "character") => ({
  id,
  name,
  type,
  updatedAt: "2026-01-01T00:00:00Z",
});

describe("InvolvedPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    commands.entityTypes.mockReturnValue([
      { key: "character", label: "Character", icon: "user" },
      { key: "faction", label: "Faction", icon: "flag" },
    ]);
    commands.listInvolved.mockResolvedValue(
      ok([summary("c1", "Aldric"), summary("f1", "House Vane", "faction")]),
    );
    commands.addInvolved.mockResolvedValue(ok(null));
    commands.removeInvolved.mockResolvedValue(ok(null));
  });

  it("lists involved entities and opens one on click", async () => {
    renderWithQuery(<InvolvedPanel eventId="ev1" />);

    fireEvent.click(await screen.findByText("House Vane"));

    expect(commands.listInvolved).toHaveBeenCalledWith("ev1");
    expect(openEntity).toHaveBeenCalledWith("f1");
    expect(screen.getByText("Aldric")).toBeInTheDocument();
  });

  it("adds the entity picked in the dialog", async () => {
    renderWithQuery(<InvolvedPanel eventId="ev1" />);
    await screen.findByText("Aldric");

    fireEvent.click(screen.getByRole("button", { name: /add/i }));
    fireEvent.click(await screen.findByText("Pick Mira"));

    await waitFor(() => expect(commands.addInvolved).toHaveBeenCalledWith("ev1", "c3"));
  });

  it("does not add someone who is already involved", async () => {
    renderWithQuery(<InvolvedPanel eventId="ev1" />);
    await screen.findByText("Aldric");
    fireEvent.click(screen.getByRole("button", { name: /add/i }));
    fireEvent.click(await screen.findByText("Pick Aldric"));

    expect(commands.addInvolved).not.toHaveBeenCalled();
  });

  it("removes an entity", async () => {
    renderWithQuery(<InvolvedPanel eventId="ev1" />);
    await screen.findByText("Aldric");

    fireEvent.click(screen.getByRole("button", { name: "Remove Aldric" }));

    await waitFor(() => expect(commands.removeInvolved).toHaveBeenCalledWith("ev1", "c1"));
  });

  it("shows an empty hint", async () => {
    commands.listInvolved.mockResolvedValue(ok([]));
    renderWithQuery(<InvolvedPanel eventId="ev1" />);
    expect(await screen.findByText(/Nobody linked yet/)).toBeInTheDocument();
  });
});
