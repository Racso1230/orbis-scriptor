import type { MapObject } from "@/lib/bindings";

/**
 * Callbacks every renderer calls. The canvas keeps them referentially stable
 * so memoised renderers do not re-render when something unrelated changes.
 */
export interface ObjectHandlers {
  onSelect: (id: string) => void;
  /** `null` = the pointer left the object. */
  onHover: (id: string | null) => void;
  /** The object was dragged by (dx, dy) map pixels. Commits an undoable move. */
  onMove: (id: string, dx: number, dy: number) => void;
  /** Double-click: open the linked entity. */
  onOpen: (object: MapObject) => void;
}

/** Props of every entry in the renderer registry (see `renderers.ts`). */
export interface ObjectRendererProps {
  object: MapObject;
  /**
   * For a label that follows a path: that path object (so the label can lay
   * itself out along it and re-render when the path changes). Otherwise undefined.
   */
  pathObject?: MapObject;
  selected: boolean;
  /** False on a locked layer: still drawn and hoverable, but cannot be selected or moved. */
  selectable: boolean;
  /** True only for the Select tool on an unlocked layer. */
  draggable: boolean;
  handlers: ObjectHandlers;
}

/** Hover text for an object, or `null` for no tooltip. */
export function hoverLabel(object: MapObject): string | null {
  if (object.entityName) return object.entityName;
  if (object.geometry.kind === "pin") return "Unlinked pin";
  return null;
}
