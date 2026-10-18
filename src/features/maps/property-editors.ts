import type { ComponentType } from "react";
import type { MapGeometry } from "@/lib/bindings";
import type { PropertyEditorProps } from "./property-editor-types";
import { LabelEditor } from "./label-editor";
import { AreaEditor, PathEditor } from "./shape-editors";
import { StampEditor } from "./stamp-editor";

/**
 * Kind-specific sections of the properties panel. Kinds without an entry
 * (pins) just show the common sections (linked entity, layer, delete).
 */
export const PROPERTY_EDITORS: Partial<
  Record<MapGeometry["kind"], ComponentType<PropertyEditorProps>>
> = {
  path: PathEditor,
  area: AreaEditor,
  label: LabelEditor,
  stamp: StampEditor,
};
