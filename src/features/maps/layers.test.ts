import { describe, expect, it } from "vitest";
import type { MapLayer, MapObject } from "@/lib/bindings";
import {
  bottomFirst,
  canEditObject,
  deleteBlockedReason,
  displayOrder,
  drawOrder,
  nextLayerName,
  reorderedIds,
  resolveActiveLayer,
} from "./layers";

const layer = (id: string, sortOrder: number, over: Partial<MapLayer> = {}): MapLayer => ({
  id,
  mapId: "m",
  name: id,
  sortOrder,
  visible: true,
  locked: false,
  ...over,
});

// Deliberately unsorted input.
const layers = [layer("top", 2), layer("bottom", 0), layer("mid", 1)];

describe("layer ordering", () => {
  it("is bottom-first for storage and top-first for display", () => {
    expect(bottomFirst(layers).map((l) => l.id)).toEqual(["bottom", "mid", "top"]);
    expect(displayOrder(layers).map((l) => l.id)).toEqual(["top", "mid", "bottom"]);
  });

  it("moves a layer up or down as displayed, as a bottom-first id list", () => {
    expect(reorderedIds(layers, "mid", "up")).toEqual(["bottom", "top", "mid"]);
    expect(reorderedIds(layers, "mid", "down")).toEqual(["mid", "bottom", "top"]);
  });

  it("refuses moves past the ends and unknown ids", () => {
    expect(reorderedIds(layers, "top", "up")).toBeNull();
    expect(reorderedIds(layers, "bottom", "down")).toBeNull();
    expect(reorderedIds(layers, "nope", "up")).toBeNull();
  });
});

describe("resolveActiveLayer", () => {
  it("keeps the preferred layer while it is editable", () => {
    expect(resolveActiveLayer(layers, "mid")?.id).toBe("mid");
  });

  it("falls back to the top-most visible unlocked layer", () => {
    const ls = [layer("a", 0), layer("b", 1, { locked: true }), layer("c", 2, { visible: false })];
    expect(resolveActiveLayer(ls, undefined)?.id).toBe("a");
    expect(resolveActiveLayer(ls, "b")?.id).toBe("a");
    expect(resolveActiveLayer(ls, "c")?.id).toBe("a");
  });

  it("returns null when nothing is editable", () => {
    expect(resolveActiveLayer([layer("a", 0, { locked: true })], "a")).toBeNull();
    expect(resolveActiveLayer([], null)).toBeNull();
  });
});

describe("object helpers", () => {
  const object = (id: string, layerId: string, createdAt: string): MapObject => ({
    id,
    layerId,
    geometry: { kind: "pin", x: 0, y: 0 },
    style: {},
    entityId: null,
    entityType: null,
    entityName: null,
    createdAt,
    updatedAt: createdAt,
  });

  it("draws a layer's objects oldest first", () => {
    const objects = [
      object("c", "L", "2026-01-03"),
      object("a", "L", "2026-01-01"),
      object("x", "other", "2026-01-00"),
      object("b", "L", "2026-01-02"),
    ];
    expect(drawOrder(objects, "L").map((o) => o.id)).toEqual(["a", "b", "c"]);
  });

  it("only allows editing objects on visible, unlocked layers", () => {
    const ls = [
      layer("ok", 0),
      layer("locked", 1, { locked: true }),
      layer("hid", 2, { visible: false }),
    ];
    expect(canEditObject(object("1", "ok", "t"), ls)).toBe(true);
    expect(canEditObject(object("2", "locked", "t"), ls)).toBe(false);
    expect(canEditObject(object("3", "hid", "t"), ls)).toBe(false);
    expect(canEditObject(object("4", "missing", "t"), ls)).toBe(false);
  });
});

describe("layer names and deletion", () => {
  it("suggests the next free name", () => {
    expect(nextLayerName([layer("a", 0, { name: "Default" })])).toBe("Layer 2");
    expect(nextLayerName([layer("a", 0, { name: "Layer 2" })])).toBe("Layer 3");
  });

  it("explains why a layer cannot be deleted", () => {
    const only = layer("a", 0);
    expect(deleteBlockedReason(only, [only])).toMatch(/at least one/);
    const locked = layer("b", 1, { locked: true });
    expect(deleteBlockedReason(locked, [only, locked])).toMatch(/Unlock/);
    expect(deleteBlockedReason(only, [only, locked])).toBeNull();
  });
});
