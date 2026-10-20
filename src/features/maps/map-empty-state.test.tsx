import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { MapEmptyState } from "./map-empty-state";

const h = vi.hoisted(() => ({ openGenerateWizard: vi.fn() }));
vi.mock("@/features/generator/generate-store", () => ({
  openGenerateWizard: h.openGenerateWizard,
}));

describe("MapEmptyState", () => {
  it("offers import, blank and generate; generate opens the wizard for this map", () => {
    const onImport = vi.fn();
    const onStartBlank = vi.fn();
    render(
      <MapEmptyState
        mapEntityId="map-3"
        importing={false}
        onImport={onImport}
        onStartBlank={onStartBlank}
      />,
    );
    expect(screen.getByText("This map has no background yet.")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Import map image" }));
    expect(onImport).toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Start blank" }));
    expect(onStartBlank).toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Generate terrain…" }));
    expect(h.openGenerateWizard).toHaveBeenCalledWith({ targetMapEntityId: "map-3" });
  });

  it("disables import while an image is being imported", () => {
    render(<MapEmptyState mapEntityId="m" importing onImport={vi.fn()} onStartBlank={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Importing…" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Generate terrain…" })).toBeEnabled();
  });
});
