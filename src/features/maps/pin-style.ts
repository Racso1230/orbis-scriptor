import type { MapObject } from "@/lib/bindings";

export interface PinLook {
  color: string;
  /** One character drawn inside the pin head. */
  glyph: string;
}

const UNLINKED: PinLook = { color: "#78716c", glyph: "?" };

// Fallback colour and glyph for the built-in types. Konva draws on a canvas, so these
// are plain hex values rather than Tailwind classes.
const LOOKS: Record<string, PinLook> = {
  location: { color: "#16a34a", glyph: "L" },
  character: { color: "#2563eb", glyph: "C" },
  faction: { color: "#dc2626", glyph: "F" },
  item: { color: "#d97706", glyph: "I" },
  note: { color: "#ca8a04", glyph: "N" },
  map: { color: "#7c3aed", glyph: "M" },
  event: { color: "#db2777", glyph: "E" },
};

/**
 * Look of a pin: by linked entity type, grey when unlinked or the entity is gone.
 * `typeLook` is the colour and glyph from the entity type definition (so custom
 * types work); without it the built-in table is the fallback (e.g. before the
 * type list has loaded).
 */
export function pinLook(
  object: Pick<MapObject, "entityId" | "entityType">,
  typeLook?: PinLook,
): PinLook {
  if (!object.entityId || !object.entityType) return UNLINKED;
  return typeLook ?? LOOKS[object.entityType] ?? { color: "#0891b2", glyph: "•" };
}
