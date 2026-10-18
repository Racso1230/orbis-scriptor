import type { MapBackground, MapLayer, MapObject } from "@/lib/bindings";

/**
 * What the canvas passes to the active tool's overlay component.
 *
 * A tool overlay is an ordinary React component rendered inside a
 * non-listening Konva `<Layer>` on top of the map. It draws its own live
 * preview and listens to the stage directly:
 *
 *   const groupRef = useRef<Konva.Group>(null);
 *   useEffect(() => {
 *     const stage = groupRef.current?.getStage();
 *     stage?.on("click.mytool tap.mytool", handler);   // namespaced
 *     return () => { stage?.off(".mytool"); };
 *   }, []);
 *
 * It commits finished work through `useMapActions()` (so it is undoable) and
 * reads the pointer with `pointerInMap(stage)`. Use `strokeScaleEnabled={false}`
 * for preview strokes and give dots the `SCREEN_FIXED` name + call
 * `applyScreenFixed(stage, scale)` after rendering to keep them a constant size.
 */
export interface ToolOverlayProps {
  mapWidth: number;
  mapHeight: number;
  /** The active layer: always an editable (visible, unlocked) one. */
  layerId: string;
  /** The map's background look (null for image maps): label tool picks a readable colour. */
  background: MapBackground | null;
  /** Every layer and object of the map, for tools that need hit-testing (label-on-path). */
  layers: MapLayer[];
  objects: MapObject[];
}
