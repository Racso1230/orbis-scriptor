import { describe, expect, it } from "vitest";
import type { MapLayer, MapObject } from "@/lib/bindings";
import {
  MAX_EXPORT_PIXELS,
  MAX_EXPORT_SIDE,
  clampExportSize,
  exportFileName,
  exportGroups,
  exportPinZoom,
  stripDataUrlPrefix,
} from "./export-png";

describe("clampExportSize", () => {
  it("uses the requested scale when it fits", () => {
    expect(clampExportSize(2000, 1000, 2)).toEqual({
      scale: 2,
      width: 4000,
      height: 2000,
      clamped: false,
    });
    expect(clampExportSize(2000, 1000, 0.5)).toMatchObject({ width: 1000, height: 500 });
  });

  it("limits the longest side", () => {
    const size = clampExportSize(10000, 1000, 4);
    expect(size.clamped).toBe(true);
    expect(size.width).toBeLessThanOrEqual(MAX_EXPORT_SIDE);
    expect(size.width).toBe(MAX_EXPORT_SIDE);
  });

  it("limits the total pixel count", () => {
    const size = clampExportSize(6000, 6000, 4);
    expect(size.clamped).toBe(true);
    expect(size.width * size.height).toBeLessThanOrEqual(MAX_EXPORT_PIXELS);
  });

  it("copes with junk scales and degenerate maps", () => {
    expect(clampExportSize(1000, 1000, 0).scale).toBe(1);
    expect(clampExportSize(1000, 1000, NaN).scale).toBe(1);
    expect(clampExportSize(0, 0, 1)).toMatchObject({ width: 1, height: 1 });
  });
});

describe("exportFileName", () => {
  it("appends .png and drops characters Windows rejects", () => {
    expect(exportFileName("Shattered Coast")).toBe("Shattered Coast.png");
    expect(exportFileName('The "Realm": North/South?')).toBe("The Realm NorthSouth.png");
  });

  it("falls back to a generic name for an empty one", () => {
    expect(exportFileName("  ")).toBe("map.png");
    expect(exportFileName("???")).toBe("map.png");
  });
});

describe("exportGroups", () => {
  const layer = (id: string, sortOrder: number, visible: boolean): MapLayer => ({
    id,
    mapId: "m",
    name: id,
    sortOrder,
    visible,
    locked: false,
  });
  const obj = (id: string, layerId: string, kind: "pin" | "stamp"): MapObject => ({
    id,
    layerId,
    geometry:
      kind === "pin"
        ? { kind: "pin", x: 0, y: 0 }
        : { kind: "stamp", x: 0, y: 0, icon: "city", scale: 1, rotation: 0 },
    style: {},
    entityId: null,
    entityType: null,
    entityName: null,
    createdAt: id,
    updatedAt: id,
  });
  const layers = [layer("top", 1, true), layer("bottom", 0, true), layer("hidden", 2, false)];
  const objects = [
    obj("a", "bottom", "pin"),
    obj("b", "bottom", "stamp"),
    obj("c", "hidden", "pin"),
  ];

  it("lists visible layers bottom first", () => {
    const groups = exportGroups(layers, objects, { includeHidden: false, includePins: true });
    expect(groups.map((g) => g.layer.id)).toEqual(["bottom", "top"]);
    expect(groups[0]?.objects.map((o) => o.id)).toEqual(["a", "b"]);
  });

  it("can include hidden layers and drop pins", () => {
    const groups = exportGroups(layers, objects, { includeHidden: true, includePins: false });
    expect(groups.map((g) => g.layer.id)).toEqual(["bottom", "top", "hidden"]);
    expect(groups.flatMap((g) => g.objects.map((o) => o.id))).toEqual(["b"]);
  });
});

describe("pin zoom and data urls", () => {
  it("enlarges pins on maps bigger than about 1400 px", () => {
    expect(exportPinZoom(700, 500)).toBe(1);
    expect(exportPinZoom(2800, 1000)).toBe(0.5);
  });

  it("strips a data URL prefix", () => {
    expect(stripDataUrlPrefix("data:image/png;base64,iVBOR")).toBe("iVBOR");
    expect(stripDataUrlPrefix("iVBOR")).toBe("iVBOR");
  });
});
