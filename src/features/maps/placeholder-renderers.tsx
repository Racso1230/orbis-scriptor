import { memo } from "react";
import { Circle, Text } from "react-konva";
import { anchorPoint } from "./geometry";
import { ObjectShell } from "./object-shell";
import { SELECTION_COLOR } from "./object-style";
import type { ObjectRendererProps } from "./render-types";

/**
 * PLACEHOLDER label: plain text at its anchor. Part 3 replaces this entry in
 * the renderer registry with real text styling and curved-along-path layout.
 */
export const LabelPlaceholder = memo(function LabelPlaceholder({
  object,
  selected,
  selectable,
  draggable,
  handlers,
}: ObjectRendererProps) {
  const g = object.geometry;
  const point = anchorPoint(g);
  if (g.kind !== "label" || !point) return null;
  const fontSize = typeof object.style["fontSize"] === "number" ? object.style["fontSize"] : 28;
  const color = typeof object.style["color"] === "string" ? object.style["color"] : "#1f2937";
  return (
    <ObjectShell
      object={object}
      anchor={point}
      rotation={g.rotation}
      selectable={selectable}
      draggable={draggable}
      handlers={handlers}
    >
      <Text
        text={g.text}
        fontSize={fontSize}
        fill={color}
        stroke={selected ? SELECTION_COLOR : undefined}
        strokeWidth={selected ? 1 : 0}
        perfectDrawEnabled={false}
      />
    </ObjectShell>
  );
});

/**
 * PLACEHOLDER stamp: a circle with the icon's first letter. Part 3 replaces
 * this entry in the renderer registry with the real icon library.
 */
export const StampPlaceholder = memo(function StampPlaceholder({
  object,
  selected,
  selectable,
  draggable,
  handlers,
}: ObjectRendererProps) {
  const g = object.geometry;
  const point = anchorPoint(g);
  if (g.kind !== "stamp" || !point) return null;
  const radius = 18 * g.scale;
  return (
    <ObjectShell
      object={object}
      anchor={point}
      rotation={g.rotation}
      selectable={selectable}
      draggable={draggable}
      handlers={handlers}
    >
      <Circle
        radius={radius}
        fill="#475569"
        stroke={selected ? SELECTION_COLOR : "#ffffff"}
        strokeWidth={selected ? 3 : 1.5}
        perfectDrawEnabled={false}
      />
      <Text
        text={g.icon.charAt(0).toUpperCase()}
        x={-radius}
        y={-radius * 0.5}
        width={radius * 2}
        align="center"
        fontSize={radius}
        fontStyle="bold"
        fill="#ffffff"
        listening={false}
        perfectDrawEnabled={false}
      />
    </ObjectShell>
  );
});
