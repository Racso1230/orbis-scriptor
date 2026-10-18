import { memo } from "react";
import { Circle } from "react-konva";
import { ObjectShell } from "./object-shell";
import { SELECTION_COLOR, isHexColor } from "./object-style";
import type { ObjectRendererProps } from "./render-types";
import { StampGlyph } from "./stamp-glyph";
import { STAMP_BASE_SIZE } from "./stamp-library";

/** A placed stamp: its glyph centred on (x, y), scaled and rotated. */
export const StampRenderer = memo(function StampRenderer({
  object,
  selected,
  selectable,
  draggable,
  handlers,
}: ObjectRendererProps) {
  const g = object.geometry;
  if (g.kind !== "stamp") return null;
  const color = isHexColor(object.style["color"]) ? object.style["color"] : null;

  return (
    <ObjectShell
      object={object}
      anchor={{ x: g.x, y: g.y }}
      rotation={g.rotation}
      selectable={selectable}
      draggable={draggable}
      handlers={handlers}
    >
      {selected && (
        <Circle
          radius={(STAMP_BASE_SIZE * g.scale) / 2}
          stroke={SELECTION_COLOR}
          strokeWidth={2}
          strokeScaleEnabled={false}
          dash={[6, 4]}
          listening={false}
        />
      )}
      <StampGlyph icon={g.icon} scale={g.scale} color={color} />
    </ObjectShell>
  );
});
