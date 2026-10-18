import {
  useCallback,
  useEffect,
  useImperativeHandle,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type Konva from "konva";
import { Group, Image as KonvaImage, Layer, Rect, Stage } from "react-konva";
import type { MapGeometry, MapInfo, MapLayer, MapObject } from "@/lib/bindings";
import { focusPoint, type Point } from "./geometry";
import { bottomFirst, drawOrder, isEditable } from "./layers";
import { DEFAULT_PLAIN_COLOR, getBackgroundCanvas } from "./map-background";
import { useMapToolStore, type MapTool } from "./map-tool-store";
import { hoverLabel, type ObjectHandlers } from "./render-types";
import { OBJECT_RENDERERS } from "./renderers";
import { applyScreenFixed } from "./screen-fixed";
import { toolById } from "./tool-registry";
import { VertexHandles } from "./vertex-handles";
import {
  MAX_ZOOM,
  centerOn,
  fitToScreen,
  screenToMap,
  zoomAround,
  type Size,
  type Transform,
} from "./viewport";

export interface MapCanvasHandle {
  fit: () => void;
  zoomBy: (direction: 1 | -1) => void;
}

interface Props {
  mapEntityId: string;
  map: MapInfo;
  layers: MapLayer[];
  /** Objects of ALL layers; the canvas draws the visible ones. */
  objects: MapObject[];
  tool: MapTool;
  selectedId: string | null;
  /** The layer new objects go to, or null when there is no editable layer. */
  activeLayer: MapLayer | null;
  onSelect: (id: string | null) => void;
  onPlacePin: (point: Point) => void;
  onMoveObject: (id: string, dx: number, dy: number) => void;
  onEditGeometry: (id: string, geometry: MapGeometry) => void;
  onOpenObject: (object: MapObject) => void;
  ref?: React.Ref<MapCanvasHandle>;
}

type ImageState =
  | { status: "none" }
  | { status: "loading" }
  | { status: "ready"; image: HTMLImageElement }
  | { status: "error" };

/** Outcome of loading one specific image path (set from the load callbacks only). */
type LoadResult = { path: string; image: HTMLImageElement | null };

/**
 * The Konva stage.
 *
 * Konva structure (four canvases in total, however many map layers exist):
 *
 *   Stage            <- pan/zoom lives here (scale + position), kept in a ref
 *    |- Layer        background (image, texture or plain colour); not listening
 *    |- Layer        content: one <Group name="map-layer"> per VISIBLE map
 *    |                 layer, bottom first, each holding that layer's objects
 *    |                 in createdAt order. Objects are drawn in MAP PIXELS, so
 *    |                 anything inside scales with zoom automatically.
 *    |- Layer        selection handles (vertex dots), Select tool only
 *    `- Layer        active tool's overlay (live preview); not listening
 *
 * One Konva Layer per map layer would mean one canvas each; groups give the
 * same stacking order for the cost of a single canvas.
 *
 * Pan/zoom is applied straight to the Konva stage with the current transform
 * kept in a ref, so a wheel notch never triggers a React render. Nodes named
 * `screen-fixed` (pins, handles, drawing dots) are counter-scaled by 1/zoom so
 * they keep their on-screen size. Labels and stamps should NOT be screen-fixed:
 * they live in map pixels and grow/shrink with the map.
 */
export function MapCanvas({
  mapEntityId,
  map,
  layers,
  objects,
  tool,
  selectedId,
  activeLayer,
  onSelect,
  onPlacePin,
  onMoveObject,
  onEditGeometry,
  onOpenObject,
  ref,
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const stageRef = useRef<Konva.Stage>(null);
  const transformRef = useRef<Transform>({ scale: 1, x: 0, y: 0 });
  const initialisedRef = useRef(false);

  const [size, setSize] = useState<Size>({ width: 0, height: 0 });
  const [hover, setHover] = useState<{ name: string; x: number; y: number } | null>(null);
  const [loaded, setLoaded] = useState<LoadResult | null>(null);

  const saveViewport = useMapToolStore((s) => s.saveViewport);
  const pendingFocus = useMapToolStore((s) => s.pendingFocus);
  const setPendingFocus = useMapToolStore((s) => s.setPendingFocus);

  const toolDef = toolById(tool);
  const Overlay = toolDef.Overlay;

  // ---- Container size ----
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const observer = new ResizeObserver(([entry]) => {
      if (!entry) return;
      const { width, height } = entry.contentRect;
      setSize({ width: Math.floor(width), height: Math.floor(height) });
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  // ---- Background image (image maps) ----
  const backgroundPath = map.background?.path ?? null;
  useEffect(() => {
    if (!backgroundPath) return;
    const img = new window.Image();
    let cancelled = false;
    img.onload = () => !cancelled && setLoaded({ path: backgroundPath, image: img });
    img.onerror = () => !cancelled && setLoaded({ path: backgroundPath, image: null });
    img.src = convertFileSrc(backgroundPath);
    return () => {
      cancelled = true;
      img.onload = null;
      img.onerror = null;
    };
  }, [backgroundPath]);

  // Derived, not stored: a result for an older path counts as "still loading".
  // Blank maps have no image to load: they are ready straight away.
  let imageState: ImageState = { status: "none" };
  if (backgroundPath) {
    if (!loaded || loaded.path !== backgroundPath) imageState = { status: "loading" };
    else if (loaded.image) imageState = { status: "ready", image: loaded.image };
    else imageState = { status: "error" };
  }

  // Procedural texture for blank maps (generated once per kind/tint/size, cached).
  const style = map.backgroundStyle;
  const texture = useMemo(
    () => (style && !backgroundPath ? getBackgroundCanvas(style, map.width, map.height) : null),
    [style, backgroundPath, map.width, map.height],
  );
  const ready = imageState.status === "ready" || (style !== null && !backgroundPath);

  // ---- Transform helpers (imperative, no React state) ----
  const applyTransform = useCallback(
    (t: Transform) => {
      const stage = stageRef.current;
      if (!stage) return;
      transformRef.current = t;
      stage.scale({ x: t.scale, y: t.scale });
      stage.position({ x: t.x, y: t.y });
      applyScreenFixed(stage, t.scale);
      stage.batchDraw();
      saveViewport(mapEntityId, t);
    },
    [mapEntityId, saveViewport],
  );

  const fit = useCallback(() => {
    applyTransform(fitToScreen({ width: map.width, height: map.height }, size));
  }, [applyTransform, map.width, map.height, size]);

  useImperativeHandle(
    ref,
    () => ({
      fit,
      zoomBy: (direction) => {
        const center = { x: size.width / 2, y: size.height / 2 };
        applyTransform(zoomAround(transformRef.current, center, -direction));
      },
    }),
    [fit, applyTransform, size],
  );

  // First load: restore the last viewport or fit; then honour a "jump to" request.
  useEffect(() => {
    if (size.width === 0 || size.height === 0 || !ready) return;
    if (!initialisedRef.current) {
      initialisedRef.current = true;
      const saved = useMapToolStore.getState().viewports[mapEntityId];
      applyTransform(saved ?? fitToScreen({ width: map.width, height: map.height }, size));
    }
    if (pendingFocus && pendingFocus.mapEntityId === mapEntityId) {
      const target = objects.find((o) => o.id === pendingFocus.objectId);
      const point = target ? focusPoint(target.geometry) : null;
      if (target && point) {
        const scale = Math.min(MAX_ZOOM, Math.max(transformRef.current.scale, 0.5));
        applyTransform(centerOn(point, size, scale));
        onSelect(target.id);
        setPendingFocus(null);
      } else if (objects.length > 0) {
        setPendingFocus(null); // the object no longer exists
      }
    }
  }, [
    size,
    ready,
    mapEntityId,
    map.width,
    map.height,
    objects,
    pendingFocus,
    applyTransform,
    onSelect,
    setPendingFocus,
  ]);

  // A different background (image, or a resize of a blank map) needs a fresh fit.
  useEffect(() => {
    initialisedRef.current = false;
  }, [backgroundPath, map.width, map.height]);

  // ---- Stage events ----
  function handleWheel(e: Konva.KonvaEventObject<WheelEvent>) {
    e.evt.preventDefault();
    const stage = stageRef.current;
    const pointer = stage?.getPointerPosition();
    if (!pointer) return;
    applyTransform(zoomAround(transformRef.current, pointer, e.evt.deltaY));
  }

  function handleStageClick(e: Konva.KonvaEventObject<MouseEvent | TouchEvent>) {
    // Objects stop propagation, so a click that reaches the stage hit empty space.
    if (e.target !== stageRef.current) return;
    if (tool === "select") {
      onSelect(null);
      return;
    }
    if (tool !== "pin") return; // drawing tools handle their own clicks
    const pointer = stageRef.current?.getPointerPosition();
    if (!pointer) return;
    const point = screenToMap(transformRef.current, pointer);
    if (point.x < 0 || point.y < 0 || point.x > map.width || point.y > map.height) return;
    onPlacePin(point);
  }

  function handleDragEnd(e: Konva.KonvaEventObject<DragEvent>) {
    // Object drags bubble up here too; only a drag of the stage itself is a pan.
    if (e.target !== stageRef.current) return;
    const stage = e.target;
    const t = { scale: transformRef.current.scale, x: stage.x(), y: stage.y() };
    transformRef.current = t;
    saveViewport(mapEntityId, t);
  }

  // ---- Stable handlers (so memoised renderers do not re-render) ----
  const objectsRef = useRef(objects);
  useEffect(() => {
    objectsRef.current = objects;
  }, [objects]);

  const handlers = useMemo<ObjectHandlers>(
    () => ({
      onSelect,
      onHover: (id) => {
        const stage = stageRef.current;
        if (stage) stage.container().style.cursor = id ? "pointer" : "";
        if (!id) {
          setHover(null);
          return;
        }
        const object = objectsRef.current.find((o) => o.id === id);
        const pointer = stage?.getPointerPosition();
        const name = object ? hoverLabel(object) : null;
        if (!name || !pointer) {
          setHover(null);
          return;
        }
        setHover({ name, x: pointer.x, y: pointer.y });
      },
      onMove: onMoveObject,
      onOpen: onOpenObject,
    }),
    [onSelect, onMoveObject, onOpenObject],
  );

  // ---- What to draw ----
  const byId = useMemo(() => new Map(objects.map((o) => [o.id, o])), [objects]);
  const layerGroups = useMemo(
    () =>
      bottomFirst(layers)
        .filter((l) => l.visible)
        .map((layer) => ({ layer, objects: drawOrder(objects, layer.id) })),
    [layers, objects],
  );

  // Newly mounted pins and handles start at scale 1; give them the counter-scale now.
  useLayoutEffect(() => {
    const stage = stageRef.current;
    if (!stage) return;
    applyScreenFixed(stage, transformRef.current.scale);
    stage.batchDraw();
  }, [layerGroups, selectedId, tool]);

  const selected = selectedId ? byId.get(selectedId) : undefined;
  const selectedLayer = selected ? layers.find((l) => l.id === selected.layerId) : undefined;
  const showHandles =
    tool === "select" &&
    selected !== undefined &&
    (selected.geometry.kind === "path" || selected.geometry.kind === "area") &&
    isEditable(selectedLayer);

  // Drawing tools take over the pointer: objects must not catch their clicks.
  const objectsListen = Overlay === undefined;

  const stageDragButtons = toolDef.leftDragPans ? [0, 1] : [1];

  return (
    <div
      ref={containerRef}
      className="relative size-full overflow-hidden bg-muted/40"
      style={{ cursor: toolDef.cursor }}
    >
      {size.width > 0 && size.height > 0 && (
        <Stage
          ref={stageRef}
          width={size.width}
          height={size.height}
          draggable
          dragButtons={stageDragButtons}
          onWheel={handleWheel}
          onClick={handleStageClick}
          onTap={handleStageClick}
          onDragEnd={handleDragEnd}
        >
          <Layer listening={false}>
            {imageState.status === "ready" && (
              <KonvaImage
                image={imageState.image}
                width={map.width}
                height={map.height}
                perfectDrawEnabled={false}
              />
            )}
            {!backgroundPath && style && texture && (
              <KonvaImage
                image={texture}
                width={map.width}
                height={map.height}
                perfectDrawEnabled={false}
              />
            )}
            {!backgroundPath && style && !texture && (
              <Rect
                width={map.width}
                height={map.height}
                fill={style.kind === "plain" ? (style.color ?? DEFAULT_PLAIN_COLOR) : "#c8b88a"}
              />
            )}
          </Layer>
          <Layer listening={objectsListen}>
            {layerGroups.map(({ layer, objects: layerObjects }) => (
              <Group key={layer.id} name="map-layer">
                {layerObjects.map((object) => {
                  const Renderer = OBJECT_RENDERERS[object.geometry.kind];
                  const pathId =
                    object.geometry.kind === "label" ? object.geometry.pathId : undefined;
                  return (
                    <Renderer
                      key={object.id}
                      object={object}
                      pathObject={pathId ? byId.get(pathId) : undefined}
                      selected={object.id === selectedId}
                      selectable={!layer.locked}
                      draggable={tool === "select" && !layer.locked}
                      handlers={handlers}
                    />
                  );
                })}
              </Group>
            ))}
          </Layer>
          <Layer listening={tool === "select"}>
            {showHandles && selected && (
              <VertexHandles key={selected.id} object={selected} onChange={onEditGeometry} />
            )}
          </Layer>
          <Layer listening={false}>
            {Overlay && activeLayer && (
              <Overlay
                key={tool}
                mapWidth={map.width}
                mapHeight={map.height}
                layerId={activeLayer.id}
              />
            )}
          </Layer>
        </Stage>
      )}

      {hover && (
        <div
          className="pointer-events-none absolute z-10 rounded-md border bg-popover px-2 py-1 text-xs text-popover-foreground shadow-md"
          style={{ left: hover.x + 14, top: hover.y + 14 }}
        >
          {hover.name}
        </div>
      )}
      {imageState.status === "loading" && (
        <p className="absolute inset-0 flex items-center justify-center text-sm text-muted-foreground">
          Loading map image…
        </p>
      )}
      {imageState.status === "error" && (
        <p
          role="alert"
          className="absolute inset-0 flex items-center justify-center p-6 text-center text-sm text-destructive"
        >
          The map image could not be loaded. Use “Change background” to pick it again.
        </p>
      )}
    </div>
  );
}
