import type { ComponentType } from "react";
import type { MapGeometry } from "@/lib/bindings";
import type { PropertyEditorProps } from "./property-editor-types";
import { AreaEditor, PathEditor } from "./shape-editors";

/**
 * Kind-specific sections of the properties panel. Part 3 adds `label` (text,
 * font size, colour) and `stamp` (icon, scale, rotation) here. Kinds without
 * an entry just show the common sections (linked entity, layer, delete).
 */
export const PROPERTY_EDITORS: Partial<
  Record<MapGeometry["kind"], ComponentType<PropertyEditorProps>>
> = {
  path: PathEditor,
  area: AreaEditor,
};
