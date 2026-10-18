import type { MapLayer, MapObject } from "@/lib/bindings";

/** Layers bottom first (the order the backend stores and draws them in). */
export function bottomFirst(layers: readonly MapLayer[]): MapLayer[] {
  return layers.slice().sort((a, b) => a.sortOrder - b.sortOrder);
}

/** Layers top first: the order the layers panel lists them in. */
export function displayOrder(layers: readonly MapLayer[]): MapLayer[] {
  return bottomFirst(layers).reverse();
}

/**
 * The id list `reorder_layers` wants (bottom first) after moving a layer one
 * step up or down *as displayed*. Returns `null` when the move is not
 * possible (already at the top/bottom, or unknown id).
 */
export function reorderedIds(
  layers: readonly MapLayer[],
  layerId: string,
  direction: "up" | "down",
): string[] | null {
  const ids = bottomFirst(layers).map((l) => l.id);
  const index = ids.indexOf(layerId);
  if (index === -1) return null;
  // "Up" on screen means towards the end of the bottom-first list.
  const target = direction === "up" ? index + 1 : index - 1;
  if (target < 0 || target >= ids.length) return null;
  const a = ids[index];
  const b = ids[target];
  if (a === undefined || b === undefined) return null;
  ids[index] = b;
  ids[target] = a;
  return ids;
}

/** A layer you can draw on: visible and not locked. */
export function isEditable(layer: MapLayer | undefined): layer is MapLayer {
  return layer !== undefined && layer.visible && !layer.locked;
}

/**
 * The layer new objects go to: the chosen one if it is still editable,
 * otherwise the top-most visible unlocked layer, otherwise `null`.
 */
export function resolveActiveLayer(
  layers: readonly MapLayer[],
  preferredId: string | null | undefined,
): MapLayer | null {
  const preferred = layers.find((l) => l.id === preferredId);
  if (isEditable(preferred)) return preferred;
  return displayOrder(layers).find((l) => isEditable(l)) ?? null;
}

/** Whether an object may be edited (its layer is visible and unlocked). */
export function canEditObject(object: MapObject, layers: readonly MapLayer[]): boolean {
  return isEditable(layers.find((l) => l.id === object.layerId));
}

/** Objects of one layer in draw order: oldest first, id as a stable tiebreak. */
export function drawOrder(objects: readonly MapObject[], layerId: string): MapObject[] {
  return objects
    .filter((o) => o.layerId === layerId)
    .sort((a, b) =>
      a.createdAt === b.createdAt ? a.id.localeCompare(b.id) : a.createdAt < b.createdAt ? -1 : 1,
    );
}

/** Default name for a new layer: "Layer N" with N one more than the count. */
export function nextLayerName(layers: readonly MapLayer[]): string {
  const names = new Set(layers.map((l) => l.name));
  let n = layers.length + 1;
  while (names.has(`Layer ${n}`)) n++;
  return `Layer ${n}`;
}

/** Why a layer cannot be deleted, or `null` when it can. */
export function deleteBlockedReason(layer: MapLayer, layers: readonly MapLayer[]): string | null {
  if (layers.length <= 1) return "A map needs at least one layer";
  if (layer.locked) return "Unlock the layer to delete it";
  return null;
}
