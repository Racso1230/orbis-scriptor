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
import { Image as KonvaImage, Layer, Stage } from "react-konva";
import type { MapInfo, MapObject } from "@/lib/bindings";
import { pinPoint, type Point } from "./geometry";
import { useMapToolStore, type MapTool } from "./map-tool-store";
import { MapPin } from "./map-pin";
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
  /** Objects of the default (first) layer. */
  objects: MapObject[];
  tool: MapTool;
  selectedId: string | null;
  onSelect: (id: string | null) => void;
  onPlacePin: (point: Point) => void;
  onMovePin: (id: string, point: Point) => void;
  onOpenPin: (object: MapObject) => void;
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
 * The Konva stage. Pan/zoom is applied straight to the Konva stage (and a pin
 * counter-scale to each pin node), with the current transform kept in a ref.
 * React state is only used for things that change rarely (size, image, hover).
 */
export function MapCanvas({
  mapEntityId,
  map,
  objects,
  tool,
  selectedId,
  onSelect,
  onPlacePin,
  onMovePin,
  onOpenPin,
  ref,
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const stageRef = useRef<Konva.Stage>(null);
  const pinLayerRef = useRef<Konva.Layer>(null);
  const transformRef = useRef<Transform>({ scale: 1, x: 0, y: 0 });
  const initialisedRef = useRef(false);

  const [size, setSize] = useState<Size>({ width: 0, height: 0 });
  const [hover, setHover] = useState<{ name: string; x: number; y: number } | null>(null);
  const [loaded, setLoaded] = useState<LoadResult | null>(null);

  const saveViewport = useMapToolStore((s) => s.saveViewport);
  const pendingFocus = useMapToolStore((s) => s.pendingFocus);
  const setPendingFocus = useMapToolStore((s) => s.setPendingFocus);

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

  // ---- Background image ----
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
  let imageState: ImageState = { status: "none" };
  if (backgroundPath) {
    if (!loaded || loaded.path !== backgroundPath) imageState = { status: "loading" };
    else if (loaded.image) imageState = { status: "ready", image: loaded.image };
    else imageState = { status: "error" };
  }

  // ---- Transform helpers (imperative, no React state) ----
  /** Applies a transform to the stage and counter-scales every pin. */
  const applyTransform = useCallback(
    (t: Transform) => {
      const stage = stageRef.current;
      if (!stage) return;
      transformRef.current = t;
      stage.scale({ x: t.scale, y: t.scale });
      stage.position({ x: t.x, y: t.y });
      const inverse = 1 / t.scale;
      const pins = pinLayerRef.current;
      if (pins) {
        for (const node of pins.getChildren()) node.scale({ x: inverse, y: inverse });
      }
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

  // First load: restore the last viewport or fit; then honour a "jump to pin" request.
  const imageReady = imageState.status === "ready";
  useEffect(() => {
    if (size.width === 0 || size.height === 0 || !imageReady) return;
    if (!initialisedRef.current) {
      initialisedRef.current = true;
      const saved = useMapToolStore.getState().viewports[mapEntityId];
      applyTransform(saved ?? fitToScreen({ width: map.width, height: map.height }, size));
    }
    if (pendingFocus && pendingFocus.mapEntityId === mapEntityId) {
      const target = objects.find((o) => o.id === pendingFocus.objectId);
      const point = target ? pinPoint(target.geometry) : null;
      if (target && point) {
        const scale = Math.min(MAX_ZOOM, Math.max(transformRef.current.scale, 0.5));
        applyTransform(centerOn(point, size, scale));
        onSelect(target.id);
        setPendingFocus(null);
      } else if (objects.length > 0) {
        setPendingFocus(null); // the pin no longer exists
      }
    }
  }, [
    size,
    imageReady,
    mapEntityId,
    map.width,
    map.height,
    objects,
    pendingFocus,
    applyTransform,
    onSelect,
    setPendingFocus,
  ]);

  // A different background image (Change background) needs a fresh fit.
  useEffect(() => {
    initialisedRef.current = false;
  }, [backgroundPath]);

  // ---- Stage events ----
  function handleWheel(e: Konva.KonvaEventObject<WheelEvent>) {
    e.evt.preventDefault();
    const stage = stageRef.current;
    const pointer = stage?.getPointerPosition();
    if (!pointer) return;
    applyTransform(zoomAround(transformRef.current, pointer, e.evt.deltaY));
  }

  function handleStageClick(e: Konva.KonvaEventObject<MouseEvent | TouchEvent>) {
    // Pins stop propagation, so a click that reaches the stage hit empty space.
    if (e.target !== stageRef.current) return;
    if (tool === "select") {
      onSelect(null);
      return;
    }
    const pointer = stageRef.current?.getPointerPosition();
    if (!pointer) return;
    const point = screenToMap(transformRef.current, pointer);
    if (point.x < 0 || point.y < 0 || point.x > map.width || point.y > map.height) return;
    onPlacePin(point);
  }

  function handleDragEnd(e: Konva.KonvaEventObject<DragEvent>) {
    // Pin drags bubble up here too; only a drag of the stage itself is a pan.
    if (e.target !== stageRef.current) return;
    const stage = e.target;
    const t = { scale: transformRef.current.scale, x: stage.x(), y: stage.y() };
    transformRef.current = t;
    saveViewport(mapEntityId, t);
  }

  // ---- Stable pin callbacks (so memoised pins do not re-render) ----
  const objectsRef = useRef(objects);
  useEffect(() => {
    objectsRef.current = objects;
  }, [objects]);

  const handleHover = useCallback((id: string | null) => {
    const stage = stageRef.current;
    if (stage) stage.container().style.cursor = id ? "pointer" : "";
    if (!id) {
      setHover(null);
      return;
    }
    const object = objectsRef.current.find((o) => o.id === id);
    const pointer = stage?.getPointerPosition();
    if (!object || !pointer) return;
    setHover({ name: object.entityName ?? "Unlinked pin", x: pointer.x, y: pointer.y });
  }, []);

  const pins = useMemo(
    () =>
      objects.flatMap((object) => {
        const point = pinPoint(object.geometry);
        return point ? [{ object, point }] : [];
      }),
    [objects],
  );

  // Newly mounted pins start at scale 1; give them the counter-scale straight away.
  useLayoutEffect(() => {
    const layer = pinLayerRef.current;
    if (!layer) return;
    const inverse = 1 / transformRef.current.scale;
    for (const node of layer.getChildren()) node.scale({ x: inverse, y: inverse });
    layer.batchDraw();
  }, [pins]);

  return (
    <div
      ref={containerRef}
      className={`relative size-full overflow-hidden bg-muted/40 ${tool === "pin" ? "cursor-crosshair" : ""}`}
    >
      {size.width > 0 && size.height > 0 && (
        <Stage
          ref={stageRef}
          width={size.width}
          height={size.height}
          // Left button and middle button both pan; pins handle their own drag.
          draggable
          dragButtons={[0, 1]}
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
          </Layer>
          <Layer ref={pinLayerRef}>
            {pins.map(({ object, point }) => (
              <MapPin
                key={object.id}
                object={object}
                point={point}
                selected={object.id === selectedId}
                draggable={tool === "select"}
                onSelect={onSelect}
                onHover={handleHover}
                onMoveEnd={onMovePin}
                onOpen={onOpenPin}
              />
            ))}
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
