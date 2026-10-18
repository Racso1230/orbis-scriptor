import { memo, useMemo } from "react";
import { Path, Rect, Text, TextPath } from "react-konva";
import type { MapObject } from "@/lib/bindings";
import { haloColor, labelLook, measureText, type LabelLook } from "./label-style";
import { ObjectShell } from "./object-shell";
import { SELECTION_COLOR, lineStyle } from "./object-style";
import { flattenSpline, orientForText, pointsToSvgPath, sliceCentered } from "./path-text";
import type { ObjectRendererProps } from "./render-types";

/** Width of the readability outline, relative to the font size. */
const HALO_RATIO = 0.22;

/**
 * A text label. If it follows a path (`pathId` set and that path exists) the
 * text is laid along the path's curve; otherwise it is straight text centred
 * on its (x, y), optionally rotated.
 */
export const LabelRenderer = memo(function LabelRenderer(props: ObjectRendererProps) {
  const { object, pathObject } = props;
  if (object.geometry.kind !== "label") return null;
  if (pathObject && pathObject.geometry.kind === "path") {
    return <CurvedLabel {...props} pathObject={pathObject} />;
  }
  return <StraightLabel {...props} />;
});

function StraightLabel({ object, selected, selectable, draggable, handlers }: ObjectRendererProps) {
  const g = object.geometry;
  const look = labelLook(object.style);
  const text = g.kind === "label" ? g.text : "";
  const width = useMemo(
    () => measureText(text, look.fontSize, look.fontFamily, look.letterSpacing),
    [text, look.fontSize, look.fontFamily, look.letterSpacing],
  );
  if (g.kind !== "label") return null;
  const pad = look.fontSize * 0.2;

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
        <Rect
          x={-width / 2 - pad}
          y={-look.fontSize / 2 - pad}
          width={width + pad * 2}
          height={look.fontSize + pad * 2}
          stroke={SELECTION_COLOR}
          strokeWidth={2}
          strokeScaleEnabled={false}
          dash={[6, 4]}
          listening={false}
        />
      )}
      <TextShapes kind="straight" text={g.text} look={look} width={width} selected={false} />
    </ObjectShell>
  );
}

function CurvedLabel({
  object,
  pathObject,
  selected,
  selectable,
  handlers,
}: ObjectRendererProps & { pathObject: MapObject }) {
  const g = object.geometry;
  const look = labelLook(object.style);
  const text = g.kind === "label" ? g.text : "";

  // Recomputed only when the label's text/font changes or the path changes
  // (the canvas keeps `pathObject` referentially stable otherwise).
  const layout = useMemo(() => {
    const pg = pathObject.geometry;
    if (pg.kind !== "path") return null;
    const smoothing = lineStyle(pathObject.style).smoothing;
    const oriented = orientForText(pg.points, pg.closed);
    const polyline = flattenSpline(oriented, smoothing, pg.closed);
    const width = measureText(text, look.fontSize, look.fontFamily, look.letterSpacing);
    // A little slack so the last letter is not clipped by rounding differences.
    const slice = sliceCentered(polyline, width * 1.03 + 2);
    return {
      data: pointsToSvgPath(slice, 0, false),
      guide: pointsToSvgPath(pg.points, smoothing, pg.closed),
    };
  }, [pathObject, text, look.fontSize, look.fontFamily, look.letterSpacing]);

  if (g.kind !== "label" || !layout || layout.data === "") return null;

  // The text sits on the path, so it cannot be dragged on its own: edit the path instead.
  return (
    <ObjectShell object={object} selectable={selectable} draggable={false} handlers={handlers}>
      {selected && (
        <Path
          data={layout.guide}
          stroke={SELECTION_COLOR}
          strokeWidth={2}
          strokeScaleEnabled={false}
          dash={[6, 4]}
          listening={false}
        />
      )}
      <TextShapes kind="curved" text={g.text} look={look} data={layout.data} selected={selected} />
    </ObjectShell>
  );
}

type TextShapesProps =
  | { kind: "straight"; text: string; look: LabelLook; width: number; selected: boolean }
  | { kind: "curved"; text: string; look: LabelLook; data: string; selected: boolean };

/** The Konva text node shared by straight and curved labels (colour, halo, spacing). */
function TextShapes(props: TextShapesProps) {
  const { text, look, selected } = props;
  const common = {
    text,
    fontSize: look.fontSize,
    fontFamily: look.fontFamily,
    fill: look.color,
    letterSpacing: look.letterSpacing,
    stroke: selected ? SELECTION_COLOR : haloColor(look.color),
    strokeWidth: look.fontSize * HALO_RATIO,
    fillAfterStrokeEnabled: true,
    lineJoin: "round" as const,
    perfectDrawEnabled: false,
  };
  if (props.kind === "straight") {
    return (
      <Text
        {...common}
        x={-props.width / 2}
        y={-look.fontSize / 2}
        wrap="none"
        hitStrokeWidth={0}
      />
    );
  }
  return <TextPath {...common} data={props.data} hitStrokeWidth={look.fontSize} />;
}
