import type { ComponentType } from "react";
import type { MapGeometry } from "@/lib/bindings";
import { AreaRenderer } from "./area-renderer";
import { MapPin } from "./map-pin";
import { PathRenderer } from "./path-renderer";
import { LabelRenderer } from "./label-renderer";
import type { ObjectRendererProps } from "./render-types";
import { StampRenderer } from "./stamp-renderer";

/**
 * The renderer registry: one memoised component per geometry kind. The canvas
 * looks up `OBJECT_RENDERERS[object.geometry.kind]` for every visible object.
 *
 * To add or replace a kind change its entry here.
 * The component receives `ObjectRendererProps` and should wrap its Konva
 * shapes in `<ObjectShell>` to get selection, hover, double-click and
 * drag-to-move for free. Wrap the component in `memo`.
 */
export const OBJECT_RENDERERS: Record<MapGeometry["kind"], ComponentType<ObjectRendererProps>> = {
  pin: MapPin,
  path: PathRenderer,
  area: AreaRenderer,
  label: LabelRenderer,
  stamp: StampRenderer,
};
