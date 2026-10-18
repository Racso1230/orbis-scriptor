import type { MapObject } from "@/lib/bindings";

/**
 * Props of a per-kind section in the properties panel (see `property-editors.ts`).
 * The panel already provides the linked-entity, layer and delete sections for
 * every kind; an editor only adds the kind-specific fields.
 */
export interface PropertyEditorProps {
  object: MapObject;
  /** True when the object's layer is locked: render controls disabled. */
  disabled: boolean;
  /**
   * Merge `changes` into the object's style as ONE undoable step.
   * (The backend replaces the whole style object, so the panel merges for you.)
   */
  onStyleChange: (label: string, changes: { [key in string]: unknown }) => void;
}
