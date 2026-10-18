import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { EntityTypeDef } from "@/lib/bindings";
import { renderWithQuery } from "@/test/render";
import { TypesPage } from "./types-page";

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const commands = vi.hoisted(() => ({
  entityTypes: vi.fn(),
  listEntities: vi.fn(),
  listTypeIcons: vi.fn(),
  createEntityType: vi.fn(),
  updateEntityType: vi.fn(),
  setTypeTemplateFields: vi.fn(),
  deleteEntityType: vi.fn(),
  reorderEntityTypes: vi.fn(),
}));
vi.mock("@/lib/bindings", () => ({ commands }));
vi.mock("@tanstack/react-router", () => ({ useBlocker: vi.fn() }));

const def = (key: string, label: string, over: Partial<EntityTypeDef> = {}): EntityTypeDef => ({
  key,
  label,
  plural: `${label}s`,
  icon: "user",
  color: "#2563eb",
  builtin: false,
  fields: [],
  ...over,
});

const CHARACTER = def("character", "Character", {
  builtin: true,
  fields: [{ key: "age", label: "Age", kind: "number", options: [], custom: false }],
});
const MAP = def("map", "Map", { builtin: true });
const DEITY = def("custom-deity", "Deity", {
  fields: [{ key: "domain", label: "Domain", kind: "select", options: ["Sea"], custom: true }],
});
const SPELL = def("custom-spell", "Spell");

describe("TypesPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    commands.entityTypes.mockResolvedValue(ok([CHARACTER, MAP, DEITY, SPELL]));
    commands.listEntities.mockResolvedValue(
      ok([
        { id: "1", type: "custom-deity", name: "Aelith", updatedAt: "" },
        { id: "2", type: "custom-deity", name: "Korrath", updatedAt: "" },
      ]),
    );
    commands.listTypeIcons.mockResolvedValue(["user", "sparkles", "flag"]);
    commands.reorderEntityTypes.mockResolvedValue(ok([CHARACTER, MAP, SPELL, DEITY]));
    commands.deleteEntityType.mockResolvedValue(ok(null));
    commands.updateEntityType.mockResolvedValue(ok(DEITY));
    commands.createEntityType.mockResolvedValue(ok(DEITY));
    commands.setTypeTemplateFields.mockResolvedValue(ok(CHARACTER));
  });

  it("lists built-in types without delete and custom types with it", async () => {
    renderWithQuery(<TypesPage />);
    expect(await screen.findByRole("button", { name: "Delete Deity" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Delete Character" })).toBeNull();
    expect(screen.getAllByText("built-in").length).toBe(2);
  });

  it("reorders custom types through the API", async () => {
    renderWithQuery(<TypesPage />);
    fireEvent.click(await screen.findByRole("button", { name: "Move Spell up" }));
    await waitFor(() =>
      expect(commands.reorderEntityTypes).toHaveBeenCalledWith(["custom-spell", "custom-deity"]),
    );
  });

  it("deletes a type with entities only after choosing where they move", async () => {
    renderWithQuery(<TypesPage />);
    fireEvent.click(await screen.findByRole("button", { name: "Delete Deity" }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText(/2 entities use this type/)).toBeInTheDocument();
    const confirm = within(dialog).getByRole("button", { name: "Move and delete" });
    expect(confirm).toBeDisabled();
    const select = within(dialog).getByLabelText(/Move 2 entities to type/);
    // Map, event and the type itself are not offered.
    expect(within(select).queryByRole("option", { name: "Map" })).toBeNull();
    expect(within(select).queryByRole("option", { name: "Deity" })).toBeNull();
    fireEvent.change(select, { target: { value: "character" } });
    fireEvent.click(confirm);
    await waitFor(() =>
      expect(commands.deleteEntityType).toHaveBeenCalledWith("custom-deity", "character"),
    );
  });

  it("deletes an unused type without a target", async () => {
    renderWithQuery(<TypesPage />);
    fireEvent.click(await screen.findByRole("button", { name: "Delete Spell" }));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "Delete" }));
    await waitFor(() =>
      expect(commands.deleteEntityType).toHaveBeenCalledWith("custom-spell", null),
    );
  });

  it("saves an edited type keeping existing field keys and sending null for new ones", async () => {
    renderWithQuery(<TypesPage />);
    fireEvent.click(await screen.findByRole("button", { name: "Edit Deity" }));
    fireEvent.click(await screen.findByRole("button", { name: "Add field" }));
    fireEvent.change(screen.getByLabelText("Label of field 2"), { target: { value: "Symbol" } });
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
    await waitFor(() => expect(commands.updateEntityType).toHaveBeenCalled());
    const args = commands.updateEntityType.mock.calls[0] as unknown[];
    expect(args[0]).toBe("custom-deity");
    const fields = args[5] as { key: string | null; label: string }[];
    expect(fields.map((f) => f.key)).toEqual(["domain", null]);
    expect(fields[1]?.label).toBe("Symbol");
  });

  it("creates a type, suggesting the plural and editing select options and references", async () => {
    renderWithQuery(<TypesPage />);
    fireEvent.click(await screen.findByRole("button", { name: "New type" }));
    fireEvent.change(await screen.findByLabelText("Name (singular)"), {
      target: { value: "Ship" },
    });
    expect(screen.getByLabelText("Plural")).toHaveValue("Ships");

    fireEvent.click(screen.getByRole("button", { name: "Add field" }));
    fireEvent.change(screen.getByLabelText("Label of field 1"), { target: { value: "Class" } });
    fireEvent.change(screen.getByLabelText("Kind of Class"), { target: { value: "select" } });
    fireEvent.click(screen.getByRole("button", { name: "Add option" }));
    fireEvent.change(screen.getByLabelText("Option 1 of Class"), { target: { value: "Galleon" } });

    fireEvent.click(screen.getByRole("button", { name: "Add field" }));
    fireEvent.change(screen.getByLabelText("Label of field 2"), { target: { value: "Owner" } });
    fireEvent.change(screen.getByLabelText("Kind of Owner"), { target: { value: "entityRef" } });
    fireEvent.click(screen.getByLabelText("Characters"));

    fireEvent.click(screen.getByRole("button", { name: "Create type" }));
    await waitFor(() => expect(commands.createEntityType).toHaveBeenCalled());
    const args = commands.createEntityType.mock.calls[0] as unknown[];
    expect(args.slice(0, 2)).toEqual(["Ship", "Ships"]);
    expect(args[4]).toEqual([
      { key: null, label: "Class", kind: "select", options: ["Galleon"] },
      {
        key: null,
        label: "Owner",
        kind: { entityRef: { allowedTypes: ["character"], multiple: false } },
        options: [],
      },
    ]);
  });

  it("offers only extra fields on a built-in type and saves through setTypeTemplateFields", async () => {
    renderWithQuery(<TypesPage />);
    fireEvent.click(await screen.findByRole("button", { name: "Edit Character" }));
    expect(await screen.findByRole("list", { name: "Built-in fields" })).toHaveTextContent("Age");
    expect(screen.queryByLabelText("Name (singular)")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Add field" }));
    fireEvent.change(screen.getByLabelText("Label of field 1"), { target: { value: "Eyes" } });
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
    await waitFor(() =>
      expect(commands.setTypeTemplateFields).toHaveBeenCalledWith("character", [
        { key: null, label: "Eyes", kind: "text", options: [] },
      ]),
    );
  });
});
