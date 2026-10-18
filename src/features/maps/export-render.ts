import { createElement } from "react";
import { flushSync } from "react-dom";
import { createRoot } from "react-dom/client";
import { convertFileSrc } from "@tauri-apps/api/core";
import Konva from "konva";
import type { MapInfo, MapLayer, MapObject } from "@/lib/bindings";
import {
  clampExportSize,
  exportGroups,
  exportPinZoom,
  stripDataUrlPrefix,
  type ExportOptions,
} from "./export-png";
import { ExportScene } from "./export-scene";
import { applyScreenFixed } from "./screen-fixed";

/** Loads the map's background picture. `crossOrigin` keeps the export canvas untainted. */
function loadImage(path: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new window.Image();
    img.crossOrigin = "anonymous";
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("The map image could not be loaded for export."));
    img.src = convertFileSrc(path);
  });
}

function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(stripDataUrlPrefix(String(reader.result)));
    reader.onerror = () => reject(new Error("The picture could not be read."));
    reader.readAsDataURL(blob);
  });
}

export interface RenderArgs {
  map: MapInfo;
  layers: MapLayer[];
  objects: MapObject[];
  options: ExportOptions;
}

/**
 * Draws the FULL map (not the current viewport) off-screen and returns it as
 * base64 PNG data (no `data:` prefix) ready for `commands.saveExport`.
 *
 * How it works: a detached react-konva stage the size of the map is rendered,
 * then `stage.toCanvas({ pixelRatio })` redraws it onto one output canvas
 * `scale` times bigger. Only that output canvas is large; the stage's own
 * layers stay map-sized. The canvas is encoded with `toBlob` (asynchronous, so
 * the window stays responsive while a big PNG is compressed).
 */
export async function renderMapToPng({
  map,
  layers,
  objects,
  options,
}: RenderArgs): Promise<string> {
  const size = clampExportSize(map.width, map.height, options.scale);
  const image = map.background ? await loadImage(map.background.path) : null;
  const groups = exportGroups(layers, objects, options);
  const byId = new Map(objects.map((o) => [o.id, o]));

  const host = document.createElement("div");
  const root = createRoot(host);
  // Konva would otherwise draw every node twice (once per layer canvas, on a
  // timer) and size those canvases for the screen's pixel ratio; neither is
  // wanted here. Both settings are restored straight afterwards.
  const previous = { autoDraw: Konva.autoDrawEnabled, pixelRatio: Konva.pixelRatio };
  Konva.autoDrawEnabled = false;
  Konva.pixelRatio = 1;
  try {
    const holder: { stage: Konva.Stage | null } = { stage: null };
    flushSync(() =>
      root.render(
        createElement(ExportScene, {
          map,
          groups,
          byId,
          image,
          onStage: (node) => {
            holder.stage = node;
          },
        }),
      ),
    );
    const stage = holder.stage;
    if (!stage) throw new Error("The export scene could not be created.");
    applyScreenFixed(stage, exportPinZoom(map.width, map.height));
    const canvas = stage.toCanvas({ pixelRatio: size.scale });
    const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/png"));
    canvas.width = 0; // free the big bitmap right away
    if (!blob) throw new Error("The picture could not be encoded as PNG.");
    return await blobToBase64(blob);
  } finally {
    Konva.autoDrawEnabled = previous.autoDraw;
    Konva.pixelRatio = previous.pixelRatio;
    root.unmount();
  }
}
