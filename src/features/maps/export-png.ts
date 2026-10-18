import type { MapLayer, MapObject } from "@/lib/bindings";
import { bottomFirst, drawOrder } from "./layers";

/** Longest side of an exported image, in pixels. */
export const MAX_EXPORT_SIDE = 16384;
/** Most pixels an export may have (8000 x 8000): bigger images freeze the app and bloat the file. */
export const MAX_EXPORT_PIXELS = 64_000_000;
/** Scales offered in the export dialog (1 = one image pixel per map pixel). */
export const EXPORT_SCALES = [0.25, 0.5, 1, 2, 4] as const;

export interface ExportSize {
  /** The scale actually used (may be lower than requested). */
  scale: number;
  width: number;
  height: number;
  /** True when the requested scale was reduced to fit the limits. */
  clamped: boolean;
}

/**
 * Output size for a map at a requested scale, reduced if the image would be
 * longer than `MAX_EXPORT_SIDE` or have more than `MAX_EXPORT_PIXELS` pixels.
 */
export function clampExportSize(mapWidth: number, mapHeight: number, scale: number): ExportSize {
  const w = Math.max(1, mapWidth);
  const h = Math.max(1, mapHeight);
  const requested = Number.isFinite(scale) && scale > 0 ? scale : 1;
  const bySide = MAX_EXPORT_SIDE / Math.max(w, h);
  const byArea = Math.sqrt(MAX_EXPORT_PIXELS / (w * h));
  const used = Math.min(requested, bySide, byArea);
  return {
    scale: used,
    width: Math.max(1, Math.floor(w * used)),
    height: Math.max(1, Math.floor(h * used)),
    clamped: used < requested,
  };
}

/** A safe default file name (`<map name>.png`) without characters Windows rejects. */
export function exportFileName(mapName: string): string {
  const cleaned = Array.from(mapName)
    .filter((ch) => ch.charCodeAt(0) >= 32 && !'<>:"/\\|?*'.includes(ch))
    .join("")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/[. ]+$/, "");
  return `${cleaned || "map"}.png`;
}

export interface ExportOptions {
  scale: number;
  includeHidden: boolean;
  includePins: boolean;
}

export interface ExportGroup {
  layer: MapLayer;
  objects: MapObject[];
}

/** The layers (bottom first) and objects that go into the picture. */
export function exportGroups(
  layers: readonly MapLayer[],
  objects: readonly MapObject[],
  options: Pick<ExportOptions, "includeHidden" | "includePins">,
): ExportGroup[] {
  return bottomFirst(layers)
    .filter((layer) => options.includeHidden || layer.visible)
    .map((layer) => ({
      layer,
      objects: drawOrder(objects, layer.id).filter(
        (o) => options.includePins || o.geometry.kind !== "pin",
      ),
    }));
}

/**
 * Zoom level pins are drawn at in an export. Pins keep a constant size on
 * screen, so on a big map they must be enlarged to look the same relative to
 * it: we pretend the map is viewed at the zoom that fits it into ~1400 screen px.
 */
export function exportPinZoom(mapWidth: number, mapHeight: number): number {
  return Math.min(1, 1400 / Math.max(mapWidth, mapHeight, 1));
}

/** Strips a `data:image/png;base64,` prefix, which the backend does not want. */
export function stripDataUrlPrefix(dataUrl: string): string {
  const comma = dataUrl.indexOf(",");
  return dataUrl.startsWith("data:") && comma >= 0 ? dataUrl.slice(comma + 1) : dataUrl;
}
