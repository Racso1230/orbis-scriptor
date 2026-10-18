import type Konva from "konva";
import { Group, Image as KonvaImage, Layer, Rect, Stage } from "react-konva";
import type { MapInfo, MapObject } from "@/lib/bindings";
import type { ExportGroup } from "./export-png";
import { DEFAULT_PLAIN_COLOR, getBackgroundCanvas } from "./map-background";
import type { ObjectHandlers } from "./render-types";
import { OBJECT_RENDERERS } from "./renderers";

const NO_HANDLERS: ObjectHandlers = {
  onSelect: () => {},
  onHover: () => {},
  onMove: () => {},
  onOpen: () => {},
};

export interface SceneProps {
  map: MapInfo;
  groups: ExportGroup[];
  byId: Map<string, MapObject>;
  image: HTMLImageElement | null;
  /** Receives the Konva stage once it exists. */
  onStage: (stage: Konva.Stage | null) => void;
}

/**
 * The whole map as a Konva scene at 1 map pixel = 1 stage pixel, using the same
 * renderers as the editor (so labels, stamps, paths and areas look identical).
 * Nothing here is interactive.
 */
export function ExportScene({ map, groups, byId, image, onStage }: SceneProps) {
  const style = map.backgroundStyle;
  const texture =
    style && !map.background ? getBackgroundCanvas(style, map.width, map.height) : null;
  return (
    <Stage ref={onStage} width={map.width} height={map.height} listening={false}>
      <Layer listening={false}>
        {image && <KonvaImage image={image} width={map.width} height={map.height} />}
        {!image && texture && <KonvaImage image={texture} width={map.width} height={map.height} />}
        {!image && !texture && (
          <Rect
            width={map.width}
            height={map.height}
            fill={style?.kind === "plain" ? (style.color ?? DEFAULT_PLAIN_COLOR) : "#c8b88a"}
          />
        )}
      </Layer>
      <Layer listening={false}>
        {groups.map(({ layer, objects }) => (
          <Group key={layer.id}>
            {objects.map((object) => {
              const Renderer = OBJECT_RENDERERS[object.geometry.kind];
              const pathId = object.geometry.kind === "label" ? object.geometry.pathId : undefined;
              return (
                <Renderer
                  key={object.id}
                  object={object}
                  pathObject={pathId ? byId.get(pathId) : undefined}
                  selected={false}
                  selectable={false}
                  draggable={false}
                  handlers={NO_HANDLERS}
                />
              );
            })}
          </Group>
        ))}
      </Layer>
    </Stage>
  );
}
