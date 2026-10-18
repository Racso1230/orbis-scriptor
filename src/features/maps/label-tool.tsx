import { useEffect, useRef } from "react";
import type Konva from "konva";
import { Group } from "react-konva";
import { useMapToolStore } from "./map-tool-store";
import { pathAt } from "./path-pick";
import { clampToMap, pointerInMap } from "./stage-pointer";
import type { ToolOverlayProps } from "./tool-types";

/**
 * Label tool: click the map to start a new label (a text box opens at that
 * spot; see `label-input.tsx`). Clicking ON a path makes the label follow it.
 * This component only turns clicks into "open the text box" requests.
 */
export function LabelTool({ mapWidth, mapHeight, layerId, layers, objects }: ToolOverlayProps) {
  const groupRef = useRef<Konva.Group>(null);
  const latest = useRef({ mapWidth, mapHeight, layerId, layers, objects });
  useEffect(() => {
    latest.current = { mapWidth, mapHeight, layerId, layers, objects };
  });

  useEffect(() => {
    const stage = groupRef.current?.getStage();
    if (!stage) return;

    function onClick() {
      if (!stage) return;
      const raw = pointerInMap(stage);
      if (!raw) return;
      const { mapWidth: w, mapHeight: h, layerId: layer, layers: ls, objects: os } = latest.current;
      if (raw.x < 0 || raw.y < 0 || raw.x > w || raw.y > h) return;
      const p = clampToMap(raw, w, h);
      const path = pathAt(p, os, ls, stage.scaleX());
      useMapToolStore.getState().setLabelInput({
        mode: "create",
        x: p.x,
        y: p.y,
        pathId: path?.id ?? null,
        layerId: layer,
      });
    }

    stage.on("click.labeltool tap.labeltool", onClick);
    return () => {
      stage.off(".labeltool");
    };
  }, []);

  return <Group ref={groupRef} listening={false} />;
}
