import { useEffect, useRef } from "react";
import type Konva from "konva";
import { Group, Line } from "react-konva";
import type { Point } from "./geometry";
import { flattenPoints } from "./geometry";
import { freehandToPoints } from "./drawing";
import { useMapActions } from "./map-actions-context";
import { useMapToolStore } from "./map-tool-store";
import { PATH_PRESETS, lineStyle } from "./object-style";
import { clampToMap, pointerInMap } from "./stage-pointer";
import type { ToolOverlayProps } from "./tool-types";

/**
 * Freehand tool: press and drag to draw. The raw mouse samples are collected
 * in a ref and drawn straight into a Konva line (no React renders per mouse
 * move). On release they are simplified with Ramer–Douglas–Peucker and saved
 * as a smoothed path. The stage only pans with the middle button while this
 * tool is active (see `MapCanvas`).
 */
export function FreehandTool({ mapWidth, mapHeight, layerId }: ToolOverlayProps) {
  const groupRef = useRef<Konva.Group>(null);
  const lineRef = useRef<Konva.Line>(null);
  const samples = useRef<Point[] | null>(null);

  const actions = useMapActions();
  const latest = useRef({ actions, layerId, mapWidth, mapHeight });
  useEffect(() => {
    latest.current = { actions, layerId, mapWidth, mapHeight };
  });

  const style = lineStyle(PATH_PRESETS.freehand.style);

  useEffect(() => {
    const stage = groupRef.current?.getStage();
    if (!stage) return;

    function addSample() {
      if (!stage || !samples.current) return;
      const raw = pointerInMap(stage);
      if (!raw) return;
      const { mapWidth: w, mapHeight: h } = latest.current;
      const p = clampToMap(raw, w, h);
      const last = samples.current[samples.current.length - 1];
      // Skip samples less than ~2 screen px from the last one.
      if (last && Math.hypot(p.x - last.x, p.y - last.y) * stage.scaleX() < 2) return;
      samples.current.push(p);
      lineRef.current?.points(flattenPoints(samples.current));
      lineRef.current?.getLayer()?.batchDraw();
    }

    function onDown(e: Konva.KonvaEventObject<MouseEvent | TouchEvent>) {
      const evt = e.evt;
      if (evt instanceof MouseEvent && evt.button !== 0) return; // middle = pan
      samples.current = [];
      addSample();
    }

    function onUp() {
      const raw = samples.current;
      samples.current = null;
      lineRef.current?.points([]);
      lineRef.current?.getLayer()?.batchDraw();
      if (!raw || !stage) return;
      const points = freehandToPoints(raw, stage.scaleX());
      if (!points) return;
      const { actions: a, layerId: layer } = latest.current;
      const [id] = a.create("Draw freehand", [
        {
          layerId: layer,
          geometry: { kind: "path", points, closed: false },
          style: PATH_PRESETS.freehand.style,
        },
      ]);
      if (id) useMapToolStore.getState().select(id);
    }

    function onKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape" && samples.current) {
        samples.current = null;
        lineRef.current?.points([]);
        lineRef.current?.getLayer()?.batchDraw();
      }
    }

    stage.on("mousedown.freehand touchstart.freehand", onDown);
    stage.on("mousemove.freehand touchmove.freehand", addSample);
    // Release anywhere, even outside the canvas.
    window.addEventListener("pointerup", onUp);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      stage.off(".freehand");
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, []);

  return (
    <Group ref={groupRef} listening={false}>
      <Line
        ref={lineRef}
        points={[]}
        tension={style.smoothing}
        stroke={style.stroke}
        strokeWidth={style.strokeWidth}
        opacity={0.8}
        lineCap="round"
        lineJoin="round"
      />
    </Group>
  );
}
