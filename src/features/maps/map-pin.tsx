import { memo } from "react";
import type Konva from "konva";
import { Circle, Group, Line, Text } from "react-konva";
import type { MapObject } from "@/lib/bindings";
import type { Point } from "./geometry";
import { pinLook } from "./pin-style";

const HEAD_RADIUS = 11;
const HEAD_Y = -22;

export interface MapPinProps {
  object: MapObject;
  point: Point;
  selected: boolean;
  draggable: boolean;
  onSelect: (id: string) => void;
  onHover: (id: string | null) => void;
  onMoveEnd: (id: string, point: Point) => void;
  onOpen: (object: MapObject) => void;
}

/**
 * One pin. The group origin is the pin's tip, so (x, y) is the exact map
 * position. Wrapped in `memo` so a wheel zoom or a change to another pin does
 * not re-render this one: the parent keeps every callback stable.
 *
 * The group's scale is 1/stageScale, which keeps the pin the same size on
 * screen at any zoom. The canvas sets that scale directly on the Konva
 * nodes (on zoom and when pins mount), never through React props or state.
 */
export const MapPin = memo(function MapPin({
  object,
  point,
  selected,
  draggable,
  onSelect,
  onHover,
  onMoveEnd,
  onOpen,
}: MapPinProps) {
  const look = pinLook(object);

  return (
    <Group
      id={object.id}
      name="pin"
      x={point.x}
      y={point.y}
      draggable={draggable}
      onClick={(e) => {
        e.cancelBubble = true;
        onSelect(object.id);
      }}
      onTap={(e) => {
        e.cancelBubble = true;
        onSelect(object.id);
      }}
      onDblClick={(e) => {
        e.cancelBubble = true;
        onOpen(object);
      }}
      onDblTap={(e) => {
        e.cancelBubble = true;
        onOpen(object);
      }}
      onMouseEnter={() => onHover(object.id)}
      onMouseLeave={() => onHover(null)}
      onDragStart={() => {
        onHover(null);
        onSelect(object.id);
      }}
      onDragEnd={(e: Konva.KonvaEventObject<DragEvent>) => {
        e.cancelBubble = true; // do not let the stage think it was panned
        onMoveEnd(object.id, { x: e.target.x(), y: e.target.y() });
      }}
    >
      {/* Tip of the teardrop. */}
      <Line
        points={[-7, HEAD_Y + 7, 0, 0, 7, HEAD_Y + 7]}
        closed
        fill={look.color}
        stroke={selected ? "#facc15" : "#ffffff"}
        strokeWidth={selected ? 3 : 1.5}
        perfectDrawEnabled={false}
      />
      <Circle
        y={HEAD_Y}
        radius={HEAD_RADIUS}
        fill={look.color}
        stroke={selected ? "#facc15" : "#ffffff"}
        strokeWidth={selected ? 3 : 1.5}
        perfectDrawEnabled={false}
      />
      <Text
        text={look.glyph}
        x={-HEAD_RADIUS}
        y={HEAD_Y - 7}
        width={HEAD_RADIUS * 2}
        align="center"
        fontSize={13}
        fontStyle="bold"
        fill="#ffffff"
        listening={false}
        perfectDrawEnabled={false}
      />
    </Group>
  );
});
