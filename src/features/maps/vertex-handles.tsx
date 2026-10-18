import { useState } from "react";
import type Konva from "konva";
import { Circle, Group, Line } from "react-konva";
import type { MapGeometry, MapObject } from "@/lib/bindings";
import { MIN_POINTS } from "./drawing";
import { flattenPoints, moveVertex, shapePoints, type Point } from "./geometry";
import { SELECTION_COLOR } from "./object-style";
import { SCREEN_FIXED } from "./screen-fixed";

interface Props {
  object: MapObject;
  /** Called once per edit (drag end / vertex removal) with the new geometry. */
  onChange: (id: string, geometry: MapGeometry) => void;
}

function midpoint(a: Point, b: Point): Point {
  return { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
}

/**
 * Handles for editing a selected path or area:
 * - drag a solid dot to move that vertex
 * - drag a faint dot (the middle of a segment) to insert a new vertex
 * - double-click a solid dot to delete that vertex
 * While dragging, a dashed outline previews the result; the edit is saved
 * (as one undoable step) when the mouse is released.
 */
export function VertexHandles({ object, onChange }: Props) {
  const [preview, setPreview] = useState<Point[] | null>(null);
  const g = object.geometry;
  const points = shapePoints(g);
  if (!points || (g.kind !== "path" && g.kind !== "area")) return null;
  const closed = g.kind === "area" || g.closed;
  const min = MIN_POINTS[g.kind];

  const mids: { index: number; p: Point }[] = [];
  const segmentCount = closed ? points.length : points.length - 1;
  for (let i = 0; i < segmentCount; i++) {
    const a = points[i];
    const b = points[(i + 1) % points.length];
    if (a && b) mids.push({ index: i, p: midpoint(a, b) });
  }

  function commit(next: Point[]) {
    setPreview(null);
    onChange(object.id, { ...g, points: next } as MapGeometry);
  }

  function dragPoint(e: Konva.KonvaEventObject<DragEvent>): Point {
    return { x: e.target.x(), y: e.target.y() };
  }

  return (
    <Group>
      {preview && (
        <Line
          points={flattenPoints(preview)}
          closed={closed}
          stroke={SELECTION_COLOR}
          strokeWidth={2}
          strokeScaleEnabled={false}
          dash={[6, 4]}
          listening={false}
        />
      )}
      {mids.map(({ index, p }) => (
        <Circle
          key={`m${index}`}
          name={SCREEN_FIXED}
          x={p.x}
          y={p.y}
          radius={4}
          fill="#ffffff"
          opacity={0.55}
          stroke="#2563eb"
          strokeWidth={1}
          strokeScaleEnabled={false}
          draggable
          onDragStart={(e) => {
            e.cancelBubble = true;
          }}
          onDragMove={(e) => {
            e.cancelBubble = true;
            const next = points.slice();
            next.splice(index + 1, 0, dragPoint(e));
            setPreview(next);
          }}
          onDragEnd={(e) => {
            e.cancelBubble = true;
            const next = points.slice();
            next.splice(index + 1, 0, dragPoint(e));
            commit(next);
          }}
        />
      ))}
      {points.map((p, i) => (
        <Circle
          key={`v${i}`}
          name={SCREEN_FIXED}
          x={p.x}
          y={p.y}
          radius={6}
          fill="#ffffff"
          stroke="#2563eb"
          strokeWidth={2}
          strokeScaleEnabled={false}
          draggable
          onDragStart={(e) => {
            e.cancelBubble = true;
          }}
          onDragMove={(e) => {
            e.cancelBubble = true;
            const moved = moveVertex(g, i, dragPoint(e));
            setPreview(shapePoints(moved));
          }}
          onDragEnd={(e) => {
            e.cancelBubble = true;
            const moved = shapePoints(moveVertex(g, i, dragPoint(e)));
            if (moved) commit(moved);
          }}
          onDblClick={(e) => {
            e.cancelBubble = true;
            if (points.length > min) commit(points.filter((_, j) => j !== i));
          }}
          onClick={(e) => {
            e.cancelBubble = true; // do not deselect when clicking a handle
          }}
        />
      ))}
    </Group>
  );
}
