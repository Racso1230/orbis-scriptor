import { memo, useMemo } from "react";
import { Line } from "react-konva";
import { flattenPoints } from "./geometry";
import { getPatternTile } from "./fill-pattern";
import { ObjectShell } from "./object-shell";
import { SELECTION_COLOR, areaStyle, withOpacity } from "./object-style";
import type { ObjectRendererProps } from "./render-types";

/**
 * A filled polygon (kingdom, region). `fill` + `fillOpacity` give a flat tint;
 * `fillPattern` "hatch" / "dots" repeats a small generated tile instead.
 */
export const AreaRenderer = memo(function AreaRenderer({
  object,
  selected,
  selectable,
  draggable,
  handlers,
}: ObjectRendererProps) {
  const g = object.geometry;
  const s = areaStyle(object.style);
  const tile = useMemo(
    () => (s.fillPattern === "none" ? null : getPatternTile(s.fillPattern, s.fill, s.fillOpacity)),
    [s.fillPattern, s.fill, s.fillOpacity],
  );
  if (g.kind !== "area") return null;
  const points = flattenPoints(g.points);

  return (
    <ObjectShell object={object} selectable={selectable} draggable={draggable} handlers={handlers}>
      <Line
        points={points}
        closed
        tension={s.smoothing}
        {...(tile
          ? {
              // Konva types this as an image, but a canvas works as a pattern source too.
              fillPatternImage: tile as unknown as HTMLImageElement,
              fillPriority: "pattern" as const,
            }
          : { fill: withOpacity(s.fill, s.fillOpacity) })}
        stroke={selected ? SELECTION_COLOR : s.stroke}
        strokeWidth={selected ? s.strokeWidth + 2 : s.strokeWidth}
        dash={!selected && s.dash.length > 0 ? s.dash : undefined}
        lineJoin="round"
        perfectDrawEnabled={false}
      />
    </ObjectShell>
  );
});
