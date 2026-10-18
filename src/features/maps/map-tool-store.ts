import { create } from "zustand";
import type { PathPreset } from "./object-style";
import type { Transform } from "./viewport";

/**
 * Tool ids. Part 3 adds "label" and "stamp" here and registers them in
 * `tool-registry.tsx`.
 */
export type MapTool = "select" | "pin" | "path" | "freehand" | "area";

/** Ask the map view to centre on a pin once it has loaded ("jump to" links). */
export interface PendingFocus {
  mapEntityId: string;
  objectId: string;
}

interface MapToolState {
  tool: MapTool;
  /** Single selection. `null` = nothing selected. */
  selectedObjectId: string | null;
  pendingFocus: PendingFocus | null;
  /** Last pan/zoom per map entity, so switching tabs keeps your place. */
  viewports: Record<string, Transform>;
  /** The layer new objects go to, per map entity (validated against the layers on use). */
  activeLayerIds: Record<string, string>;
  /** Style preset for the Path tool. */
  pathPreset: PathPreset;
  /** Whether the layers / properties side panel is shown. */
  panelOpen: boolean;
  setTool: (tool: MapTool) => void;
  select: (objectId: string | null) => void;
  setPendingFocus: (focus: PendingFocus | null) => void;
  saveViewport: (mapEntityId: string, transform: Transform) => void;
  clearViewport: (mapEntityId: string) => void;
  setActiveLayer: (mapEntityId: string, layerId: string) => void;
  setPathPreset: (preset: PathPreset) => void;
  setPanelOpen: (open: boolean) => void;
}

export const useMapToolStore = create<MapToolState>()((set) => ({
  tool: "select",
  selectedObjectId: null,
  pendingFocus: null,
  viewports: {},
  activeLayerIds: {},
  pathPreset: "river",
  panelOpen: true,
  // Switching tool also clears the selection: objects are only edited in Select.
  setTool: (tool) => set({ tool, selectedObjectId: null }),
  select: (selectedObjectId) => set({ selectedObjectId }),
  setPendingFocus: (pendingFocus) => set({ pendingFocus }),
  saveViewport: (mapEntityId, transform) =>
    set((s) => ({ viewports: { ...s.viewports, [mapEntityId]: transform } })),
  clearViewport: (mapEntityId) =>
    set((s) => {
      const viewports = { ...s.viewports };
      delete viewports[mapEntityId];
      return { viewports };
    }),
  setActiveLayer: (mapEntityId, layerId) =>
    set((s) => ({ activeLayerIds: { ...s.activeLayerIds, [mapEntityId]: layerId } })),
  setPathPreset: (pathPreset) => set({ pathPreset }),
  setPanelOpen: (panelOpen) => set({ panelOpen }),
}));
