import { memo } from "react";
import { Circle, Line, Text } from "react-konva";
import { anchorPoint } from "./geometry";
import { ObjectShell } from "./object-shell";
import { SELECTION_COLOR } from "./object-style";
import { pinLook } from "./pin-style";
import type { ObjectRendererProps } from "./render-types";

const HEAD_RADIUS = 11;
const HEAD_Y = -22;

/**
 * One pin. The shell origin is the pin's tip, so (x, y) is the exact map
 * position. Wrapped in `memo` so a wheel zoom or a change to another pin does
 * not re-render this one: the canvas keeps every callback stable.
 *
 * The group's scale is 1/stageScale, which keeps the pin the same size on
 * screen at any zoom. The canvas sets that scale directly on the Konva
 * nodes (on zoom and when pins mount), never through React props or state.
 */
export const MapPin = memo(function MapPin({
  object,
  selected,
  selectable,
  draggable,
  handlers,
}: ObjectRendererProps) {
  const point = anchorPoint(object.geometry);
  const look = pinLook(object);
  if (!point) return null;

  return (
    <ObjectShell
      object={object}
      anchor={point}
      screenFixed
      selectable={selectable}
      draggable={draggable}
      handlers={handlers}
    >
      {/* Tip of the teardrop. */}
      <Line
        points={[-7, HEAD_Y + 7, 0, 0, 7, HEAD_Y + 7]}
        closed
        fill={look.color}
        stroke={selected ? SELECTION_COLOR : "#ffffff"}
        strokeWidth={selected ? 3 : 1.5}
        perfectDrawEnabled={false}
      />
      <Circle
        y={HEAD_Y}
        radius={HEAD_RADIUS}
        fill={look.color}
        stroke={selected ? SELECTION_COLOR : "#ffffff"}
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
    </ObjectShell>
  );
});
