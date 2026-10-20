import { useCallback, useEffect, useRef, useState } from "react";
import {
  Download,
  FlaskConical,
  ImagePlus,
  Maximize,
  PanelRight,
  Redo2,
  Settings2,
  Undo2,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { useCreateEntity } from "@/features/entities/queries";
import { useOpenEntity } from "@/features/entities/use-open-entity";
import type { MapGeometry, MapLayer, MapObject } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { BlankMapDialog } from "./blank-map-dialog";
import { EntityPickerDialog } from "./entity-picker-dialog";
import { ExportDialog } from "./export-dialog";
import { pinGeometry, translateGeometry, type Point } from "./geometry";
import { LayersPanel } from "./layers-panel";
import { canEditObject, resolveActiveLayer } from "./layers";
import { MapActionsProvider } from "./map-actions";
import { useMapActions } from "./map-actions-context";
import { MapCanvas, type MapCanvasHandle } from "./map-canvas";
import { MapEmptyState } from "./map-empty-state";
import { MapSettingsDialog } from "./map-settings-dialog";
import { useMapToolStore, type MapTool } from "./map-tool-store";
import { PATH_PRESETS, type PathPreset } from "./object-style";
import { PropertiesPanel } from "./properties-panel";
import { StampPalette } from "./stamp-palette";
import { pickImageFile, useMap, useSeedMapPins, useSetMapBackground } from "./queries";
import { isTypingTarget } from "./stage-pointer";
import { TOOLS, toolForKey, toolById } from "./tool-registry";

const SEED_PINS = 500;
const PRESET_IDS = Object.keys(PATH_PRESETS).filter((k) => k !== "freehand") as PathPreset[];

/**
 * The editor for an entity of type `map`. Loads the map and provides the
 * undoable-actions context; the editor UI itself is `MapEditor`.
 */
export function MapView({ mapEntityId }: { mapEntityId: string }) {
  const [error, setError] = useState<string | null>(null);

  // Do not carry a selection or the active tool over to the next map you open.
  // (The undo history is deliberately kept per map, in memory, across tab switches.)
  useEffect(() => {
    return () => {
      useMapToolStore.setState({ selectedObjectId: null, tool: "select" });
    };
  }, [mapEntityId]);

  return (
    <MapActionsProvider mapEntityId={mapEntityId} onError={setError} error={error}>
      <MapEditor mapEntityId={mapEntityId} />
    </MapActionsProvider>
  );
}

function MapEditor({ mapEntityId }: { mapEntityId: string }) {
  const map = useMap(mapEntityId);
  const actions = useMapActions();
  const canvasRef = useRef<MapCanvasHandle>(null);

  const tool = useMapToolStore((s) => s.tool);
  const setTool = useMapToolStore((s) => s.setTool);
  const selectedId = useMapToolStore((s) => s.selectedObjectId);
  const select = useMapToolStore((s) => s.select);
  const activeLayerId = useMapToolStore((s) => s.activeLayerIds[mapEntityId]);
  const setActiveLayer = useMapToolStore((s) => s.setActiveLayer);
  const pathPreset = useMapToolStore((s) => s.pathPreset);
  const setPathPreset = useMapToolStore((s) => s.setPathPreset);
  const panelOpen = useMapToolStore((s) => s.panelOpen);
  const setPanelOpen = useMapToolStore((s) => s.setPanelOpen);

  const setBackground = useSetMapBackground(mapEntityId);
  const seedPins = useSeedMapPins(mapEntityId);
  const createEntity = useCreateEntity();
  const openEntity = useOpenEntity();

  // Where the Pin tool was clicked; the pin is only created once an entity is chosen.
  const [pendingPin, setPendingPin] = useState<Point | null>(null);
  const [blankOpen, setBlankOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);

  const data = map.data;
  const layers = data?.layers ?? [];
  const objects = data?.objects ?? [];
  const activeLayer = resolveActiveLayer(layers, activeLayerId);
  const selected = selectedId ? objects.find((o) => o.id === selectedId) : undefined;
  const hasCanvas = Boolean(data?.map.background || data?.map.backgroundStyle);
  const isBlank = Boolean(data?.map.backgroundStyle) && !data?.map.background;

  // Drop a selection whose object is gone (deleted, or undone away).
  useEffect(() => {
    if (selectedId && data && !data.objects.some((o) => o.id === selectedId)) select(null);
  }, [selectedId, data, select]);

  // A tool that needs an editable layer cannot stay active without one.
  useEffect(() => {
    if (activeLayer === null && toolById(tool).needsEditableLayer) setTool("select");
  }, [activeLayer, tool, setTool]);

  // ---- Keyboard shortcuts (undo/redo are global: see use-global-shortcuts) ----
  const { remove, getObjects } = actions;
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.ctrlKey || e.metaKey || e.altKey || isTypingTarget(e.target)) return;
      const hit = toolForKey(e.key);
      if (hit) {
        if (!hasCanvas || (hit.needsEditableLayer && activeLayer === null)) return;
        setTool(hit.id);
      } else if (e.key === "Escape") {
        select(null);
      } else if (
        (e.key === "Delete" || e.key === "Backspace") &&
        selectedId &&
        // Backspace means "remove last point" while drawing, so only act in Select/Pin.
        (tool === "select" || tool === "pin")
      ) {
        const layersNow = data?.layers ?? [];
        const object = getObjects().find((o) => o.id === selectedId);
        if (!object || !canEditObject(object, layersNow)) return;
        e.preventDefault();
        remove(`Delete ${object.geometry.kind}`, [selectedId]);
        select(null);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [setTool, select, selectedId, tool, hasCanvas, activeLayer, remove, getObjects, data?.layers]);

  async function importBackground() {
    setImportError(null);
    try {
      const path = await pickImageFile();
      if (!path) return;
      await setBackground.mutateAsync(path);
      useMapToolStore.getState().clearViewport(mapEntityId);
    } catch (e) {
      setImportError(e instanceof Error ? e.message : "Could not import the image.");
    }
  }

  function placePin(entityId: string) {
    const point = pendingPin;
    setPendingPin(null);
    if (!point || !activeLayer) return;
    // Stay in the Pin tool so several pins can be dropped in a row.
    actions.create("Add pin", [
      { layerId: activeLayer.id, geometry: pinGeometry(point), style: null, entityId },
    ]);
  }

  async function createAndPlace(type: string, name: string) {
    try {
      const entity = await createEntity.mutateAsync({ type, name });
      placePin(entity.id);
    } catch {
      setPendingPin(null); // the global toast already explained the failure
    }
  }

  // Stable callbacks so memoised renderers do not re-render needlessly.
  const { update, getObjects: readObjects } = actions;
  const moveObject = useCallback(
    (id: string, dx: number, dy: number) => {
      const object = readObjects().find((o) => o.id === id);
      if (!object) return;
      update(`Move ${object.geometry.kind}`, [
        { id, patch: { geometry: translateGeometry(object.geometry, dx, dy) } },
      ]);
    },
    [update, readObjects],
  );
  const editGeometry = useCallback(
    (id: string, geometry: MapGeometry) => {
      update("Edit shape", [{ id, patch: { geometry } }]);
    },
    [update],
  );
  // Read through a ref so `openObject` stays stable when layers change.
  const layersRef = useRef<MapLayer[]>([]);
  useEffect(() => {
    layersRef.current = data?.layers ?? [];
  }, [data?.layers]);
  const openObject = useCallback(
    (object: MapObject) => {
      // Double-clicking a label edits its text (if its layer can be edited).
      if (object.geometry.kind === "label" && canEditObject(object, layersRef.current)) {
        useMapToolStore.getState().setLabelInput({ mode: "edit", objectId: object.id });
      } else if (object.entityId) {
        openEntity(object.entityId);
      }
    },
    [openEntity],
  );

  if (map.isPending) return <p className="p-4 text-sm text-muted-foreground">Loading map…</p>;
  if (map.error || !data) {
    return <p className="p-4 text-sm text-destructive">{map.error?.message ?? "Map not found."}</p>;
  }

  const importing = setBackground.isPending;
  const error = actions.error ?? importError;
  const noEditableLayer = hasCanvas && activeLayer === null;

  return (
    <div className="flex h-[70vh] min-h-96 flex-col overflow-hidden rounded-lg border">
      <div
        role="toolbar"
        aria-label="Map tools"
        className="flex flex-wrap items-center gap-1 border-b p-1.5"
      >
        {TOOLS.map((t) => (
          <ToolButton
            key={t.id}
            active={tool === t.id}
            label={`${t.label} (${t.shortcut})`}
            onClick={() => setTool(t.id)}
            icon={<t.icon />}
            disabled={!hasCanvas || (t.needsEditableLayer && activeLayer === null)}
          />
        ))}
        {tool === "path" && (
          <select
            aria-label="Path type"
            value={pathPreset}
            onChange={(e) => setPathPreset(e.target.value as PathPreset)}
            className="h-7 rounded-md border bg-background px-1.5 text-xs"
          >
            {PRESET_IDS.map((id) => (
              <option key={id} value={id}>
                {PATH_PRESETS[id].label}
              </option>
            ))}
          </select>
        )}
        <span className="mx-1 h-5 w-px bg-border" />
        <Button
          variant="ghost"
          size="icon"
          aria-label="Undo"
          title="Undo (Ctrl+Z)"
          disabled={!actions.canUndo}
          onClick={actions.undo}
        >
          <Undo2 />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Redo"
          title="Redo (Ctrl+Shift+Z)"
          disabled={!actions.canRedo}
          onClick={actions.redo}
        >
          <Redo2 />
        </Button>
        <span className="mx-1 h-5 w-px bg-border" />
        <Button
          variant="ghost"
          size="icon"
          aria-label="Zoom in"
          disabled={!hasCanvas}
          onClick={() => canvasRef.current?.zoomBy(1)}
        >
          <ZoomIn />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Zoom out"
          disabled={!hasCanvas}
          onClick={() => canvasRef.current?.zoomBy(-1)}
        >
          <ZoomOut />
        </Button>
        <Button
          variant="ghost"
          size="sm"
          disabled={!hasCanvas}
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
          {noEditableLayer && (
            <span className="mr-2 text-xs text-muted-foreground">
              Show or unlock a layer to draw.
            </span>
          )}
          {import.meta.env.DEV && hasCanvas && (
            <Button
              variant="ghost"
              size="sm"
              disabled={seedPins.isPending}
              onClick={() => seedPins.mutate(SEED_PINS)}
            >
              <FlaskConical /> Seed {SEED_PINS} pins (dev)
            </Button>
          )}
          {isBlank && (
            <Button variant="ghost" size="sm" onClick={() => setSettingsOpen(true)}>
              <Settings2 /> Map settings
            </Button>
          )}
          {hasCanvas && !isBlank && (
            <Button variant="ghost" size="sm" disabled={importing} onClick={importBackground}>
              <ImagePlus /> {importing ? "Importing…" : "Change background"}
            </Button>
          )}
          {hasCanvas && (
            <Button variant="ghost" size="sm" onClick={() => setExportOpen(true)}>
              <Download /> Export PNG…
            </Button>
          )}
          {hasCanvas && (
            <Button
              variant={panelOpen ? "secondary" : "ghost"}
              size="icon"
              aria-label="Layers and properties"
              aria-pressed={panelOpen}
              title="Layers and properties"
              onClick={() => setPanelOpen(!panelOpen)}
            >
              <PanelRight />
            </Button>
          )}
        </div>
      </div>

      <div className="flex min-h-0 flex-1">
        <div className="relative min-h-0 min-w-0 flex-1">
          {hasCanvas ? (
            <MapCanvas
              ref={canvasRef}
              mapEntityId={mapEntityId}
              map={data.map}
              layers={data.layers}
              objects={data.objects}
              tool={tool}
              selectedId={selected ? selected.id : null}
              activeLayer={activeLayer}
              onSelect={select}
              onPlacePin={setPendingPin}
              onMoveObject={moveObject}
              onEditGeometry={editGeometry}
              onOpenObject={openObject}
            />
          ) : (
            <MapEmptyState
              mapEntityId={mapEntityId}
              importing={importing}
              onImport={() => void importBackground()}
              onStartBlank={() => setBlankOpen(true)}
            />
          )}
          {hasCanvas && objects.length === 0 && (
            <p className="pointer-events-none absolute inset-x-0 bottom-4 text-center text-sm text-muted-foreground">
              <span className="rounded-md bg-background/80 px-3 py-1.5">
                Nothing drawn yet. Pick a tool: P pin, L path, F freehand, A area, T label, S stamp.
              </span>
            </p>
          )}
          {hasCanvas && tool === "stamp" && <StampPalette />}
          {hasCanvas && tool !== "select" && (
            <p className="pointer-events-none absolute top-2 left-2 rounded-md bg-background/80 px-2 py-1 text-xs text-muted-foreground">
              {toolHint(tool)}
            </p>
          )}
        </div>

        {hasCanvas && panelOpen && (
          <aside
            aria-label="Layers and properties"
            className={cn("flex w-64 shrink-0 flex-col gap-4 overflow-y-auto border-l p-3")}
          >
            <LayersPanel
              mapEntityId={mapEntityId}
              layers={data.layers}
              activeLayerId={activeLayer?.id ?? null}
              onActivate={(id) => setActiveLayer(mapEntityId, id)}
            />
            {selected && (
              <PropertiesPanel key={selected.id} object={selected} layers={data.layers} />
            )}
          </aside>
        )}
      </div>

      <EntityPickerDialog
        open={pendingPin !== null}
        onOpenChange={(open) => {
          // Dismissing the picker discards the pin: nothing was saved yet.
          if (!open) setPendingPin(null);
        }}
        onPickEntity={placePin}
        onCreateEntity={(type, name) => void createAndPlace(type, name)}
      />
      <ExportDialog
        open={exportOpen}
        onOpenChange={setExportOpen}
        map={data.map}
        layers={data.layers}
        objects={data.objects}
      />
      <BlankMapDialog
        open={blankOpen}
        onOpenChange={setBlankOpen}
        target={{ mode: "configure", mapEntityId }}
      />
      {isBlank && (
        <MapSettingsDialog map={data.map} open={settingsOpen} onOpenChange={setSettingsOpen} />
      )}
    </div>
  );
}

function toolHint(tool: MapTool): string {
  switch (tool) {
    case "pin":
      return "Click the map to place a pin";
    case "path":
      return "Click to add points. Double-click or Enter to finish, Backspace to undo a point, Esc to cancel";
    case "freehand":
      return "Press and drag to draw. Middle-drag pans";
    case "label":
      return "Click the map to add a label. Click on a path to make the text follow it. Enter to save, Esc to cancel";
    case "stamp":
      return "Pick a stamp, then click the map to place it. Esc to stop";
    case "area":
      return "Click to add corners. Click the first corner, double-click or Enter to close, Esc to cancel";
    default:
      return "";
  }
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
