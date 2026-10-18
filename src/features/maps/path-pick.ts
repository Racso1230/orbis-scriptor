import type { MapLayer, MapObject } from "@/lib/bindings";
import type { Point } from "./geometry";
import { lineStyle } from "./object-style";
import { distanceToPolyline, flattenSpline } from "./path-text";

/** How close (screen px) a click must be to a path's line to attach the label to it. */
export const PATH_PICK_RADIUS_PX = 12;

/**
 * The visible path under `point`, if any: the nearest one within the pick
 * radius (the radius is in screen pixels, so it feels the same at any zoom).
 */
export function pathAt(
  point: Point,
  objects: readonly MapObject[],
  layers: readonly MapLayer[],
  scale: number,
): MapObject | null {
  const visible = new Set(layers.filter((l) => l.visible).map((l) => l.id));
  let best: MapObject | null = null;
  let bestDistance = Infinity;
  for (const o of objects) {
    const g = o.geometry;
    if (g.kind !== "path" || !visible.has(o.layerId)) continue;
    const style = lineStyle(o.style);
    const reach = PATH_PICK_RADIUS_PX / scale + style.strokeWidth / 2;
    const d = distanceToPolyline(point, flattenSpline(g.points, style.smoothing, g.closed, 8));
    if (d <= reach && d < bestDistance) {
      best = o;
      bestDistance = d;
    }
  }
  return best;
}
