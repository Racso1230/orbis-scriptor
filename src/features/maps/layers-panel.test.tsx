import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { MapLayer } from "@/lib/bindings";
import { renderWithQuery } from "@/test/render";
import { LayersPanel } from "./layers-panel";

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const commands = vi.hoisted(() => ({
  createLayer: vi.fn(),
  renameLayer: vi.fn(),
  setLayerVisibility: vi.fn(),
  setLayerLocked: vi.fn(),
  reorderLayers: vi.fn(),
  deleteLayer: vi.fn(),
}));

vi.mock("@/lib/bindings", () => ({ commands }));

const layer = (id: string, sortOrder: number, over: Partial<MapLayer> = {}): MapLayer => ({
  id,
  mapId: "m",
  name: id,
  sortOrder,
  visible: true,
  locked: false,
  ...over,
});

// Bottom "Default", then "Borders" on top.
const TWO = [layer("Default", 0), layer("Borders", 1)];

function renderPanel(layers: MapLayer[], activeLayerId: string | null = "Borders") {
  const onActivate = vi.fn();
  renderWithQuery(
    <LayersPanel
      mapEntityId="map-1"
      layers={layers}
      activeLayerId={activeLayerId}
      onActivate={onActivate}
    />,
  );
  return { onActivate };
}

describe("LayersPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    commands.setLayerVisibility.mockImplementation(async (id: string, visible: boolean) =>
      ok(layer(id, 0, { visible })),
    );
    commands.setLayerLocked.mockImplementation(async (id: string, locked: boolean) =>
      ok(layer(id, 0, { locked })),
    );
    commands.renameLayer.mockImplementation(async (id: string, name: string) =>
      ok(layer(id, 0, { name })),
    );
    commands.reorderLayers.mockResolvedValue(ok(TWO));
    commands.deleteLayer.mockResolvedValue(ok(null));
    commands.createLayer.mockResolvedValue(ok(layer("Layer 3", 2, { name: "Layer 3" })));
  });

  it("lists layers top first and marks the active one", () => {
    renderPanel(TWO);
    const items = screen.getAllByRole("listitem");
    expect(items.map((li) => li.textContent)).toEqual(["Borders", "Default"]);
    expect(items[0]).toHaveAttribute("aria-current", "true");
    expect(items[1]).not.toHaveAttribute("aria-current");
  });

  it("makes a layer active when its name is clicked", () => {
    const { onActivate } = renderPanel(TWO);
    fireEvent.click(screen.getByRole("button", { name: "Default" }));
    expect(onActivate).toHaveBeenCalledWith("Default");
  });

  it("toggles visibility and lock", async () => {
    renderPanel(TWO);
    fireEvent.click(screen.getByRole("button", { name: "Hide layer Borders" }));
    await waitFor(() => expect(commands.setLayerVisibility).toHaveBeenCalledWith("Borders", false));
    fireEvent.click(screen.getByRole("button", { name: "Lock layer Default" }));
    await waitFor(() => expect(commands.setLayerLocked).toHaveBeenCalledWith("Default", true));
  });

  it("renames a layer inline on Enter", async () => {
    renderPanel(TWO);
    fireEvent.click(screen.getByRole("button", { name: "Rename layer Default" }));
    const input = screen.getByRole("textbox", { name: /Name of layer Default/ });
    fireEvent.change(input, { target: { value: "Terrain" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(commands.renameLayer).toHaveBeenCalledWith("Default", "Terrain"));
  });

  it("does not rename on Escape or when the name is unchanged", () => {
    renderPanel(TWO);
    fireEvent.click(screen.getByRole("button", { name: "Rename layer Default" }));
    const input = screen.getByRole("textbox", { name: /Name of layer Default/ });
    fireEvent.change(input, { target: { value: "Other" } });
    fireEvent.keyDown(input, { key: "Escape" });
    expect(commands.renameLayer).not.toHaveBeenCalled();
  });

  it("reorders with a bottom-first id list", async () => {
    renderPanel(TWO);
    fireEvent.click(screen.getByRole("button", { name: "Move layer Borders down" }));
    await waitFor(() =>
      expect(commands.reorderLayers).toHaveBeenCalledWith("map-1", ["Borders", "Default"]),
    );
  });

  it("disables moving past the ends", () => {
    renderPanel(TWO);
    expect(screen.getByRole("button", { name: "Move layer Borders up" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Move layer Default down" })).toBeDisabled();
  });

  it("adds a layer with the next free name and activates it", async () => {
    const { onActivate } = renderPanel(TWO);
    fireEvent.click(screen.getByRole("button", { name: "Add layer" }));
    await waitFor(() => expect(commands.createLayer).toHaveBeenCalledWith("map-1", "Layer 3"));
    await waitFor(() => expect(onActivate).toHaveBeenCalledWith("Layer 3"));
  });

  it("asks for confirmation before deleting a layer", async () => {
    renderPanel(TWO);
    fireEvent.click(screen.getByRole("button", { name: "Delete layer Borders" }));
    expect(commands.deleteLayer).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole("button", { name: "Delete" }));
    await waitFor(() => expect(commands.deleteLayer).toHaveBeenCalledWith("Borders"));
  });

  it("cancelling the confirmation keeps the layer", async () => {
    renderPanel(TWO);
    fireEvent.click(screen.getByRole("button", { name: "Delete layer Borders" }));
    fireEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(commands.deleteLayer).not.toHaveBeenCalled();
  });

  it("disables delete for the last layer", () => {
    renderPanel([layer("Default", 0)], "Default");
    expect(screen.getByRole("button", { name: "Delete layer Default" })).toBeDisabled();
  });

  it("disables delete for a locked layer", () => {
    renderPanel([layer("Default", 0), layer("Borders", 1, { locked: true })]);
    expect(screen.getByRole("button", { name: "Delete layer Borders" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Delete layer Default" })).toBeEnabled();
    // The lock button offers to unlock it.
    expect(screen.getByRole("button", { name: "Unlock layer Borders" })).toBeInTheDocument();
  });

  it("shows a backend error inline", async () => {
    commands.setLayerVisibility.mockResolvedValue({
      status: "error",
      error: { kind: "validation", message: "Nope" },
    });
    renderPanel(TWO);
    fireEvent.click(screen.getByRole("button", { name: "Hide layer Borders" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Nope");
  });
});
