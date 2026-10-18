import { describe, expect, it } from "vitest";
import type { MapLayer, MapObject } from "@/lib/bindings";
import { pathAt } from "./path-pick";

const layer = (id: string, visible: boolean): MapLayer => ({
  id,
  mapId: "m",
  name: id,
  sortOrder: 0,
  visible,
  locked: false,
});

const path = (id: string, layerId: string, y: number): MapObject => ({
  id,
  layerId,
  geometry: {
    kind: "path",
    closed: false,
    points: [
      { x: 0, y },
      { x: 200, y },
    ],
  },
  style: { strokeWidth: 4 },
  entityId: null,
  entityType: null,
  entityName: null,
  createdAt: id,
  updatedAt: id,
});

describe("pathAt", () => {
  const layers = [layer("a", true), layer("hidden", false)];
  const objects = [path("near", "a", 100), path("far", "a", 140), path("ghost", "hidden", 100)];

  it("finds the path under the pointer", () => {
    expect(pathAt({ x: 50, y: 104 }, objects, layers, 1)?.id).toBe("near");
  });

  it("returns the nearest of two paths", () => {
    expect(pathAt({ x: 50, y: 128 }, objects, layers, 1)?.id).toBe("far");
  });

  it("ignores empty space, hidden layers and non-paths", () => {
    expect(pathAt({ x: 50, y: 300 }, objects, layers, 1)).toBeNull();
    expect(pathAt({ x: 50, y: 100 }, [path("ghost", "hidden", 100)], layers, 1)).toBeNull();
  });

  it("uses a screen-sized pick radius, so it grows when zoomed out", () => {
    // 30 map px away: out of reach at 1x, within reach at 0.2x zoom (12 px = 60 map px).
    expect(pathAt({ x: 50, y: 130 }, [path("p", "a", 100)], layers, 1)).toBeNull();
    expect(pathAt({ x: 50, y: 130 }, [path("p", "a", 100)], layers, 0.2)?.id).toBe("p");
  });
});
