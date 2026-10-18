import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type Konva from "konva";
import { Circle, Group, Line } from "react-konva";
import type { Point } from "./geometry";
import { flattenPoints } from "./geometry";
import { appendPoint, canFinish, isNearFirst, type PolylineKind } from "./drawing";
import { useMapActions } from "./map-actions-context";
import { useMapToolStore } from "./map-tool-store";
import {
  DEFAULT_AREA_STYLE,
  PATH_PRESETS,
  lineStyle,
  withOpacity,
  areaStyle,
} from "./object-style";
import { SCREEN_FIXED, applyScreenFixed } from "./screen-fixed";
import { clampToMap, isTypingTarget, pointerInMap } from "./stage-pointer";
import type { ToolOverlayProps } from "./tool-types";

const PREVIEW_BLUE = "#2563eb";

/**
 * Click-to-add-points drawing, shared by the Path and Area tools.
 *
 * - click: add a point
 * - double-click or Enter: finish (Area: also click the first point)
 * - Esc: cancel, Backspace: remove the last point
 *
 * Points live in React state (they change a few times per second), and the
 * finished shape is committed as one undoable "create".
 */
function PolylineTool({
  kind,
  mapWidth,
  mapHeight,
  layerId,
}: ToolOverlayProps & { kind: PolylineKind }) {
  const groupRef = useRef<Konva.Group>(null);
  const [points, setPoints] = useState<Point[]>([]);
  // The pointer in map pixels, with the zoom it was read at (to size the close-hint).
  const [cursor, setCursor] = useState<{ p: Point; scale: number } | null>(null);
  // The event handlers below are attached once, so they read the latest
  // values through this ref instead of capturing stale state.
  const latest = useRef({ points, layerId, mapWidth, mapHeight });

  const actions = useMapActions();
  const actionsRef = useRef(actions);
  const preset = useMapToolStore((s) => s.pathPreset);
  const presetRef = useRef(preset);

  useEffect(() => {
    latest.current = { points, layerId, mapWidth, mapHeight };
    actionsRef.current = actions;
    presetRef.current = preset;
  });

  useEffect(() => {
    const stage = groupRef.current?.getStage();
    if (!stage) return;
    const ns = `.${kind}tool`;

    function finish(finalPoints: Point[]) {
      if (!canFinish(kind, finalPoints)) return;
      const { layerId: layer } = latest.current;
      const isArea = kind === "area";
      const presetStyle = PATH_PRESETS[presetRef.current];
      const [id] = actionsRef.current.create(isArea ? "Draw area" : `Draw ${presetStyle.label}`, [
        {
          layerId: layer,
          geometry: isArea
            ? { kind: "area", points: finalPoints }
            : { kind: "path", points: finalPoints, closed: false },
          style: isArea ? DEFAULT_AREA_STYLE : presetStyle.style,
        },
      ]);
      // Select the new shape so its properties (and entity link) are one click away.
      if (id) useMapToolStore.getState().select(id);
      setPoints([]);
    }

    function onClick() {
      const current = stage?.getPointerPosition() ? pointerInMap(stage) : null;
      if (!stage || !current) return;
      const { points: pts, mapWidth: w, mapHeight: h } = latest.current;
      const p = clampToMap(current, w, h);
      if (kind === "area" && pts.length >= 3 && isNearFirst(pts, p, stage.scaleX())) {
        finish(pts);
        return;
      }
      setPoints(appendPoint(pts, p, 0.5 / stage.scaleX()));
    }

    function onDblClick() {
      finish(latest.current.points);
    }

    function onMove() {
      if (!stage) return;
      const p = pointerInMap(stage);
      setCursor(
        p
          ? {
              p: clampToMap(p, latest.current.mapWidth, latest.current.mapHeight),
              scale: stage.scaleX(),
            }
          : null,
      );
    }

    function onKeyDown(e: KeyboardEvent) {
      if (isTypingTarget(e.target) || e.ctrlKey || e.metaKey || e.altKey) return;
      const pts = latest.current.points;
      if (e.key === "Enter") {
        e.preventDefault();
        finish(pts);
      } else if (e.key === "Escape") {
        setPoints([]);
      } else if (e.key === "Backspace" && pts.length > 0) {
        e.preventDefault();
        setPoints(pts.slice(0, -1));
      }
    }

    stage.on(`click${ns} tap${ns}`, onClick);
    stage.on(`dblclick${ns} dbltap${ns}`, onDblClick);
    stage.on(`mousemove${ns}`, onMove);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      stage.off(ns);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [kind]);

  // Dots are created after the last zoom; give them the counter-scale now.
  useLayoutEffect(() => {
    const stage = groupRef.current?.getStage();
    if (stage) applyScreenFixed(stage, stage.scaleX());
  });

  const live = cursor && points.length > 0 ? [...points, cursor.p] : points;
  const style =
    kind === "area" ? areaStyle(DEFAULT_AREA_STYLE) : lineStyle(PATH_PRESETS[preset].style);
  const closeHint =
    kind === "area" && points.length >= 3 && cursor !== null
      ? isNearFirst(points, cursor.p, cursor.scale)
      : false;

  return (
    <Group ref={groupRef} listening={false}>
      {live.length >= 2 &&
        (kind === "area" ? (
          <Line
            points={flattenPoints(live)}
            closed
            fill={withOpacity(PREVIEW_BLUE, 0.15)}
            stroke={PREVIEW_BLUE}
            strokeWidth={2}
            strokeScaleEnabled={false}
            dash={[6, 4]}
          />
        ) : (
          <Line
            points={flattenPoints(live)}
            tension={style.smoothing}
            stroke={style.stroke}
            strokeWidth={style.strokeWidth}
            dash={style.dash.length > 0 ? style.dash : undefined}
            opacity={0.8}
            lineCap="round"
            lineJoin="round"
          />
        ))}
      {points.map((p, i) => (
        <Circle
          key={i}
          name={SCREEN_FIXED}
          x={p.x}
          y={p.y}
          radius={i === 0 && closeHint ? 8 : 4}
          fill={i === 0 && closeHint ? PREVIEW_BLUE : "#ffffff"}
          stroke={PREVIEW_BLUE}
          strokeWidth={2}
          strokeScaleEnabled={false}
        />
      ))}
    </Group>
  );
}

/** Path tool: rivers, roads and borders (style comes from the toolbar preset). */
export function PathTool(props: ToolOverlayProps) {
  return <PolylineTool kind="path" {...props} />;
}

/** Area tool: kingdoms and regions. */
export function AreaTool(props: ToolOverlayProps) {
  return <PolylineTool kind="area" {...props} />;
}
