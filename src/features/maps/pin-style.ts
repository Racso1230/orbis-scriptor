import type { MapObject } from "@/lib/bindings";

export interface PinLook {
  color: string;
  /** One character drawn inside the pin head. */
  glyph: string;
}

const UNLINKED: PinLook = { color: "#78716c", glyph: "?" };

// Colour and glyph per built-in entity type. Konva draws on a canvas, so these
// are plain hex values rather than Tailwind classes.
const LOOKS: Record<string, PinLook> = {
  location: { color: "#16a34a", glyph: "L" },
  character: { color: "#2563eb", glyph: "C" },
  faction: { color: "#dc2626", glyph: "F" },
  item: { color: "#d97706", glyph: "I" },
  note: { color: "#ca8a04", glyph: "N" },
  map: { color: "#7c3aed", glyph: "M" },
};

/** Look of a pin: by linked entity type, grey when unlinked or the entity is gone. */
export function pinLook(object: Pick<MapObject, "entityId" | "entityType">): PinLook {
  if (!object.entityId || !object.entityType) return UNLINKED;
  return LOOKS[object.entityType] ?? { color: "#0891b2", glyph: "•" };
}
