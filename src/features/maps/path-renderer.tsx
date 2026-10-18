import { memo } from "react";
import { Line } from "react-konva";
import { flattenPoints } from "./geometry";
import { ObjectShell } from "./object-shell";
import { SELECTION_COLOR, lineStyle } from "./object-style";
import type { ObjectRendererProps } from "./render-types";

/** Minimum width (map px) of the invisible hit area, so thin rivers are easy to click. */
const MIN_HIT_WIDTH = 16;

/**
 * Rivers, roads, borders and freehand strokes. Stroke width and dashes are in
 * map pixels, so they scale with zoom like the map itself. `smoothing` is
 * passed to Konva as the curve tension (0 = straight segments).
 */
export const PathRenderer = memo(function PathRenderer({
  object,
  selected,
  selectable,
  draggable,
  handlers,
}: ObjectRendererProps) {
  const g = object.geometry;
  if (g.kind !== "path") return null;
  const s = lineStyle(object.style);
  const points = flattenPoints(g.points);

  return (
    <ObjectShell object={object} selectable={selectable} draggable={draggable} handlers={handlers}>
      {selected && (
        <Line
          points={points}
          tension={s.smoothing}
          closed={g.closed}
          stroke={SELECTION_COLOR}
          strokeWidth={s.strokeWidth + 6}
          opacity={0.55}
          lineCap="round"
          lineJoin="round"
          listening={false}
          perfectDrawEnabled={false}
        />
      )}
      <Line
        points={points}
        tension={s.smoothing}
        closed={g.closed}
        stroke={s.stroke}
        strokeWidth={s.strokeWidth}
        dash={s.dash.length > 0 ? s.dash : undefined}
        lineCap="round"
        lineJoin="round"
        hitStrokeWidth={Math.max(s.strokeWidth, MIN_HIT_WIDTH)}
        perfectDrawEnabled={false}
      />
    </ObjectShell>
  );
});
