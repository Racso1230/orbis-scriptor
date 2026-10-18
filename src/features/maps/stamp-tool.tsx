import { useEffect, useRef, useState } from "react";
import type Konva from "konva";
import { Group } from "react-konva";
import type { Point } from "./geometry";
import { useMapActions } from "./map-actions-context";
import { useMapToolStore } from "./map-tool-store";
import { StampGlyph } from "./stamp-glyph";
import { defaultStampScale } from "./stamp-library";
import { clampToMap, pointerInMap } from "./stage-pointer";
import type { ToolOverlayProps } from "./tool-types";

/**
 * Stamp tool: pick a stamp in the palette (see `stamp-palette.tsx`), then
 * click the map to place it. The tool stays active for repeated placement;
 * Esc goes back to the Select tool. A faint copy of the stamp follows the pointer.
 */
export function StampTool({ mapWidth, mapHeight, layerId }: ToolOverlayProps) {
  const groupRef = useRef<Konva.Group>(null);
  const [cursor, setCursor] = useState<Point | null>(null);
  const icon = useMapToolStore((s) => s.stampIcon);
  const actions = useMapActions();
  const latest = useRef({ mapWidth, mapHeight, layerId, actions });
  useEffect(() => {
    latest.current = { mapWidth, mapHeight, layerId, actions };
  });

  useEffect(() => {
    const stage = groupRef.current?.getStage();
    if (!stage) return;

    function onClick() {
      if (!stage) return;
      const raw = pointerInMap(stage);
      const { mapWidth: w, mapHeight: h, layerId: layer, actions: a } = latest.current;
      if (!raw || raw.x < 0 || raw.y < 0 || raw.x > w || raw.y > h) return;
      const p = clampToMap(raw, w, h);
      const key = useMapToolStore.getState().stampIcon;
      const [id] = a.create("Add stamp", [
        {
          layerId: layer,
          geometry: {
            kind: "stamp",
            x: p.x,
            y: p.y,
            icon: key,
            scale: defaultStampScale(w),
            rotation: 0,
          },
          style: null,
        },
      ]);
      if (id) useMapToolStore.getState().select(id);
    }

    function onMove() {
      if (!stage) return;
      const p = pointerInMap(stage);
      setCursor(p ? clampToMap(p, latest.current.mapWidth, latest.current.mapHeight) : null);
    }

    function onKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") useMapToolStore.getState().setTool("select");
    }

    stage.on("click.stamptool tap.stamptool", onClick);
    stage.on("mousemove.stamptool", onMove);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      stage.off(".stamptool");
      window.removeEventListener("keydown", onKeyDown);
    };
  }, []);

  return (
    <Group ref={groupRef} listening={false}>
      {cursor && (
        <Group x={cursor.x} y={cursor.y}>
          <StampGlyph
            icon={icon}
            scale={defaultStampScale(mapWidth)}
            color={null}
            opacity={0.6}
            listening={false}
          />
        </Group>
      )}
    </Group>
  );
}
