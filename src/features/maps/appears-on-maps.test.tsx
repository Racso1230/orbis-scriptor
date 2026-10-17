import { fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithQuery } from "@/test/render";
import { AppearsOnMaps } from "./appears-on-maps";
import { useMapToolStore } from "./map-tool-store";

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const commands = vi.hoisted(() => ({ listEntityMapAppearances: vi.fn() }));
const openEntity = vi.hoisted(() => vi.fn());

vi.mock("@/lib/bindings", () => ({ commands }));
vi.mock("@/features/entities/use-open-entity", () => ({ useOpenEntity: () => openEntity }));

describe("AppearsOnMaps", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useMapToolStore.setState({ pendingFocus: null });
  });

  it("lists maps and jumps to the pin on click", async () => {
    commands.listEntityMapAppearances.mockResolvedValue(
      ok([
        {
          mapEntityId: "map-1",
          mapName: "Aerth",
          objectId: "pin-1",
          geometry: { kind: "pin", x: 1, y: 2 },
        },
        {
          mapEntityId: "map-2",
          mapName: "Old Coast",
          objectId: "pin-2",
          geometry: { kind: "pin", x: 3, y: 4 },
        },
      ]),
    );
    renderWithQuery(<AppearsOnMaps entityId="e1" />);

    fireEvent.click(await screen.findByText("Old Coast"));

    expect(commands.listEntityMapAppearances).toHaveBeenCalledWith("e1");
    expect(openEntity).toHaveBeenCalledWith("map-2");
    expect(useMapToolStore.getState().pendingFocus).toEqual({
      mapEntityId: "map-2",
      objectId: "pin-2",
    });
    expect(screen.getByText("Aerth")).toBeInTheDocument();
  });

  it("shows an empty hint when the entity is not on any map", async () => {
    commands.listEntityMapAppearances.mockResolvedValue(ok([]));
    renderWithQuery(<AppearsOnMaps entityId="e1" />);
    expect(await screen.findByText(/Not pinned on any map yet/)).toBeInTheDocument();
  });
});
