import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithQuery } from "@/test/render";
import { RelationshipsPanel } from "./relationships-panel";
import { KINDS } from "./test-fixtures";

const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const fail = (message: string) => ({
  status: "error" as const,
  error: { kind: "validation", message },
});

const commands = vi.hoisted(() => ({
  listRelationships: vi.fn(),
  listRelationshipKinds: vi.fn(),
  createRelationship: vi.fn(),
  updateRelationship: vi.fn(),
  deleteRelationship: vi.fn(),
  entityTypes: vi.fn(),
  listEntities: vi.fn(),
  createEntity: vi.fn(),
}));
const openEntity = vi.hoisted(() => vi.fn());
const openGraph = vi.hoisted(() => vi.fn());
const pickerProps = vi.hoisted(() => ({ allowedTypes: undefined as string[] | undefined }));

vi.mock("@/lib/bindings", () => ({ commands }));
vi.mock("@/features/entities/use-open-entity", () => ({ useOpenEntity: () => openEntity }));
vi.mock("./use-open-graph", () => ({ useOpenGraph: () => openGraph }));
// The real picker searches the entity list; here it offers one fixed choice.
vi.mock("@/features/maps/entity-picker-dialog", () => ({
  EntityPickerDialog: ({
    open,
    onPickEntity,
    allowedTypes,
  }: {
    open: boolean;
    onPickEntity: (id: string) => void;
    allowedTypes?: string[];
  }) => {
    pickerProps.allowedTypes = allowedTypes;
    return open ? <button onClick={() => onPickEntity("c2")}>Pick Aldous</button> : null;
  },
}));

const view = (over: Record<string, unknown>) => ({
  linkId: "l1",
  kind: "parent_of",
  label: "child of",
  otherId: "c2",
  otherName: "Aldous",
  otherType: "character",
  outgoing: false,
  symmetric: false,
  category: "family",
  ...over,
});

const record = { id: "new", sourceId: "", targetId: "", kind: "", label: null };

describe("RelationshipsPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    commands.entityTypes.mockReturnValue([
      { key: "character", label: "Character", plural: "Characters", icon: "user", fields: [] },
      { key: "faction", label: "Faction", plural: "Factions", icon: "flag", fields: [] },
    ]);
    commands.listRelationshipKinds.mockReturnValue(KINDS);
    commands.listEntities.mockResolvedValue(
      ok([
        { id: "c1", name: "Isolde", type: "character", updatedAt: "" },
        { id: "c2", name: "Aldous", type: "character", updatedAt: "" },
        { id: "f1", name: "House Vane", type: "faction", updatedAt: "" },
      ]),
    );
    commands.listRelationships.mockResolvedValue(
      ok([
        view({}),
        view({
          linkId: "l2",
          kind: "member_of",
          label: "member of",
          otherId: "f1",
          otherName: "House Vane",
          otherType: "faction",
          outgoing: true,
          category: "political",
        }),
      ]),
    );
    commands.createRelationship.mockResolvedValue(ok(record));
    commands.updateRelationship.mockResolvedValue(ok(record));
    commands.deleteRelationship.mockResolvedValue(ok(null));
  });

  it("lists relationships by category, worded from this entity's side", async () => {
    renderWithQuery(<RelationshipsPanel entityId="c1" entityType="character" />);

    expect(await screen.findByText("child of")).toBeInTheDocument();
    expect(screen.getByText("member of")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Family" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Political" })).toBeInTheDocument();
    expect(commands.listRelationships).toHaveBeenCalledWith("c1");
  });

  it("opens the other entity when its name is clicked", async () => {
    renderWithQuery(<RelationshipsPanel entityId="c1" entityType="character" />);
    fireEvent.click(await screen.findByRole("button", { name: "House Vane" }));
    expect(openEntity).toHaveBeenCalledWith("f1");
  });

  it("opens the graph focused on this entity", async () => {
    renderWithQuery(<RelationshipsPanel entityId="c1" entityType="character" />);
    fireEvent.click(await screen.findByRole("button", { name: /show in graph/i }));
    expect(openGraph).toHaveBeenCalledWith("c1");
  });

  it("adds an inverse choice with the other entity as the source", async () => {
    renderWithQuery(<RelationshipsPanel entityId="c1" entityType="character" />);
    await screen.findByText("child of");

    fireEvent.click(screen.getByRole("button", { name: /add relationship/i }));
    fireEvent.change(screen.getByLabelText("Relationship"), {
      target: { value: "parent_of:inverse" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Choose entity" }));
    // The picker is restricted to the types the kind allows on the other side.
    expect(pickerProps.allowedTypes).toEqual(["character"]);
    fireEvent.click(await screen.findByText("Pick Aldous"));
    fireEvent.click(screen.getByRole("button", { name: "Add" }));

    await waitFor(() =>
      expect(commands.createRelationship).toHaveBeenCalledWith("c2", "c1", "parent_of", null),
    );
  });

  it("adds a forward choice with this entity as the source", async () => {
    renderWithQuery(<RelationshipsPanel entityId="c1" entityType="character" />);
    await screen.findByText("child of");

    fireEvent.click(screen.getByRole("button", { name: /add relationship/i }));
    fireEvent.change(screen.getByLabelText("Relationship"), {
      target: { value: "spouse_of:forward" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Choose entity" }));
    fireEvent.click(await screen.findByText("Pick Aldous"));
    fireEvent.click(screen.getByRole("button", { name: "Add" }));

    await waitFor(() =>
      expect(commands.createRelationship).toHaveBeenCalledWith("c1", "c2", "spouse_of", null),
    );
  });

  it("requires a label for custom relationships and sends it", async () => {
    renderWithQuery(<RelationshipsPanel entityId="c1" entityType="character" />);
    await screen.findByText("child of");

    fireEvent.click(screen.getByRole("button", { name: /add relationship/i }));
    fireEvent.change(screen.getByLabelText("Relationship"), {
      target: { value: "custom:forward" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Choose entity" }));
    fireEvent.click(await screen.findByText("Pick Aldous"));
    expect(screen.getByRole("button", { name: "Add" })).toBeDisabled();

    fireEvent.change(screen.getByLabelText("Custom label"), { target: { value: "covets" } });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));

    await waitFor(() =>
      expect(commands.createRelationship).toHaveBeenCalledWith("c1", "c2", "custom", "covets"),
    );
  });

  it("shows backend validation errors inline", async () => {
    commands.createRelationship.mockResolvedValue(fail("Isolde is already spouse of Aldous"));
    renderWithQuery(<RelationshipsPanel entityId="c1" entityType="character" />);
    await screen.findByText("child of");

    fireEvent.click(screen.getByRole("button", { name: /add relationship/i }));
    fireEvent.change(screen.getByLabelText("Relationship"), {
      target: { value: "spouse_of:forward" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Choose entity" }));
    fireEvent.click(await screen.findByText("Pick Aldous"));
    fireEvent.click(screen.getByRole("button", { name: "Add" }));

    expect(await screen.findByText("Isolde is already spouse of Aldous")).toBeInTheDocument();
  });

  it("deletes a relationship", async () => {
    renderWithQuery(<RelationshipsPanel entityId="c1" entityType="character" />);
    fireEvent.click(await screen.findByRole("button", { name: "Delete relationship with Aldous" }));
    await waitFor(() => expect(commands.deleteRelationship).toHaveBeenCalledWith("l1"));
  });

  it("edits the direction of a relationship", async () => {
    renderWithQuery(<RelationshipsPanel entityId="c1" entityType="character" />);
    fireEvent.click(await screen.findByRole("button", { name: "Edit relationship with Aldous" }));
    fireEvent.click(screen.getByLabelText("Reverse direction"));
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(commands.updateRelationship).toHaveBeenCalledWith("l1", null, null, true),
    );
  });

  it("shows an empty hint", async () => {
    commands.listRelationships.mockResolvedValue(ok([]));
    renderWithQuery(<RelationshipsPanel entityId="c1" entityType="character" />);
    expect(await screen.findByText(/No relationships yet/)).toBeInTheDocument();
  });
});
