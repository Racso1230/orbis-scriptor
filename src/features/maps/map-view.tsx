import { useCallback, useEffect, useRef, useState } from "react";
import {
  FlaskConical,
  ImagePlus,
  MapPin,
  Maximize,
  MousePointer2,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { useCreateEntity } from "@/features/entities/queries";
import { useOpenEntity } from "@/features/entities/use-open-entity";
import type { MapObject } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { EntityPickerDialog } from "./entity-picker-dialog";
import { pinGeometry, type Point } from "./geometry";
import { MapCanvas, type MapCanvasHandle } from "./map-canvas";
import { useMapToolStore, type MapTool } from "./map-tool-store";
import {
  pickImageFile,
  useCreateMapObject,
  useDeleteMapObject,
  useMap,
  useSeedMapPins,
  useSetMapBackground,
  useUpdateMapObject,
} from "./queries";

const SEED_PINS = 500;

/** True when the keyboard event came from somewhere the user is typing. */
function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
}

/** The editor for an entity of type `map`: toolbar + canvas + pin picker. */
export function MapView({ mapEntityId }: { mapEntityId: string }) {
  const map = useMap(mapEntityId);
  const canvasRef = useRef<MapCanvasHandle>(null);

  const tool = useMapToolStore((s) => s.tool);
  const setTool = useMapToolStore((s) => s.setTool);
  const selectedId = useMapToolStore((s) => s.selectedObjectId);
  const select = useMapToolStore((s) => s.select);

  const setBackground = useSetMapBackground(mapEntityId);
  const createObject = useCreateMapObject(mapEntityId);
  const updateObject = useUpdateMapObject(mapEntityId);
  const deleteObject = useDeleteMapObject(mapEntityId);
  const seedPins = useSeedMapPins(mapEntityId);
  const createEntity = useCreateEntity();
  const openEntity = useOpenEntity();

  // Where the Pin tool was clicked; the pin is only created once an entity is chosen.
  const [pendingPin, setPendingPin] = useState<Point | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Do not carry a selection or the Pin tool over to the next map you open.
  useEffect(() => {
    return () => {
      useMapToolStore.setState({ selectedObjectId: null, tool: "select" });
    };
  }, [mapEntityId]);

  // Default layer: lowest sort order. (Layer UI arrives in a later phase.)
  const layer = map.data?.layers.slice().sort((a, b) => a.sortOrder - b.sortOrder)[0];
  const objects = (map.data?.objects ?? []).filter((o) => o.layerId === layer?.id);

  const hasBackground = Boolean(map.data?.map.background);

  // ---- Keyboard shortcuts ----
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.ctrlKey || e.metaKey || e.altKey || isTypingTarget(e.target)) return;
      if (e.key === "v" || e.key === "V") setTool("select");
      else if (e.key === "p" || e.key === "P") setTool("pin");
      else if (e.key === "Escape") select(null);
      else if ((e.key === "Delete" || e.key === "Backspace") && selectedId) {
        e.preventDefault();
        deleteObject.mutate(selectedId, { onError: (err) => setError(err.message) });
        select(null);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [setTool, select, selectedId, deleteObject]);

  async function importBackground() {
    setError(null);
    try {
      const path = await pickImageFile();
      if (!path) return;
      await setBackground.mutateAsync(path);
      useMapToolStore.getState().clearViewport(mapEntityId);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not import the image.");
    }
  }

  async function placePin(entityId: string) {
    const point = pendingPin;
    setPendingPin(null);
    if (!point || !layer) return;
    try {
      // Stay in the Pin tool so several pins can be dropped in a row.
      await createObject.mutateAsync({
        layerId: layer.id,
        geometry: pinGeometry(point),
        style: null,
        entityId,
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not place the pin.");
    }
  }

  async function createAndPlace(type: string, name: string) {
    try {
      const entity = await createEntity.mutateAsync({ type, name });
      await placePin(entity.id);
    } catch (e) {
      setPendingPin(null);
      setError(e instanceof Error ? e.message : "Could not create the entity.");
    }
  }

  // Stable callbacks so memoised pins do not re-render needlessly.
  const movePin = useCallback(
    (id: string, point: Point) => {
      updateObject.mutate(
        { id, patch: { geometry: pinGeometry(point), style: null, entityId: null } },
        { onError: (err) => setError(err.message) },
      );
    },
    // `mutate` is stable across renders in TanStack Query.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [updateObject.mutate],
  );
  const openPin = useCallback(
    (object: MapObject) => {
      if (object.entityId) openEntity(object.entityId);
    },
    [openEntity],
  );

  if (map.isPending) return <p className="p-4 text-sm text-muted-foreground">Loading map…</p>;
  if (map.error || !map.data) {
    return <p className="p-4 text-sm text-destructive">{map.error?.message ?? "Map not found."}</p>;
  }

  const importing = setBackground.isPending;

  return (
    <div className="flex h-[70vh] min-h-96 flex-col overflow-hidden rounded-lg border">
      <div role="toolbar" aria-label="Map tools" className="flex items-center gap-1 border-b p-1.5">
        <ToolButton
          active={tool === "select"}
          label="Select (V)"
          onClick={() => setTool("select")}
          icon={<MousePointer2 />}
        />
        <ToolButton
          active={tool === "pin"}
          label="Pin (P)"
          onClick={() => setTool("pin")}
          icon={<MapPin />}
          disabled={!hasBackground}
        />
        <span className="mx-1 h-5 w-px bg-border" />
        <Button
          variant="ghost"
          size="icon"
          aria-label="Zoom in"
          disabled={!hasBackground}
          onClick={() => canvasRef.current?.zoomBy(1)}
        >
          <ZoomIn />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Zoom out"
          disabled={!hasBackground}
          onClick={() => canvasRef.current?.zoomBy(-1)}
        >
          <ZoomOut />
        </Button>
        <Button
          variant="ghost"
          size="sm"
          disabled={!hasBackground}
          onClick={() => canvasRef.current?.fit()}
        >
          <Maximize /> Fit to screen
        </Button>
        <div className="ml-auto flex items-center gap-1">
          {error && (
            <span role="alert" className="mr-2 text-xs text-destructive">
              {error}
            </span>
          )}
          {import.meta.env.DEV && hasBackground && (
            <Button
              variant="ghost"
              size="sm"
              disabled={seedPins.isPending}
              onClick={() => seedPins.mutate(SEED_PINS)}
            >
              <FlaskConical /> Seed {SEED_PINS} pins (dev)
            </Button>
          )}
          {hasBackground && (
            <Button variant="ghost" size="sm" disabled={importing} onClick={importBackground}>
              <ImagePlus /> {importing ? "Importing…" : "Change background"}
            </Button>
          )}
        </div>
      </div>

      <div className="relative min-h-0 flex-1">
        {hasBackground ? (
          <MapCanvas
            ref={canvasRef}
            mapEntityId={mapEntityId}
            map={map.data.map}
            objects={objects}
            tool={tool}
            selectedId={selectedId}
            onSelect={select}
            onPlacePin={setPendingPin}
            onMovePin={movePin}
            onOpenPin={openPin}
          />
        ) : (
          <div className="flex size-full flex-col items-center justify-center gap-3 text-muted-foreground">
            <p className="text-sm">This map has no image yet.</p>
            <Button onClick={importBackground} disabled={importing}>
              <ImagePlus /> {importing ? "Importing…" : "Import map image"}
            </Button>
          </div>
        )}
      </div>

      <EntityPickerDialog
        open={pendingPin !== null}
        onOpenChange={(open) => {
          // Dismissing the picker discards the pin: nothing was saved yet.
          if (!open) setPendingPin(null);
        }}
        onPickEntity={(id) => void placePin(id)}
        onCreateEntity={(type, name) => void createAndPlace(type, name)}
      />
    </div>
  );
}

function ToolButton({
  active,
  label,
  icon,
  onClick,
  disabled,
}: {
  active: boolean;
  label: string;
  icon: React.ReactNode;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <Button
      variant={active ? "secondary" : "ghost"}
      size="icon"
      aria-label={label}
      aria-pressed={active}
      title={label}
      disabled={disabled}
      onClick={onClick}
      className={cn(active && "ring-1 ring-ring")}
    >
      {icon}
    </Button>
  );
}

export type { MapTool };
