import { create } from "zustand";
import type { PathPreset } from "./object-style";
import type { Transform } from "./viewport";

/** Tool ids. Each one is registered in `tool-registry.tsx`. */
export type MapTool = "select" | "pin" | "path" | "freehand" | "area" | "label" | "stamp";

/**
 * The inline text box for labels: `create` is a new label about to be placed at
 * (x, y), optionally following a path; `edit` changes the text of an existing one.
 */
export type LabelInput =
  | { mode: "create"; x: number; y: number; pathId: string | null; layerId: string }
  | { mode: "edit"; objectId: string };

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
  /** Open label text box, if any. */
  labelInput: LabelInput | null;
  /** The stamp (icon key) the Stamp tool places. */
  stampIcon: string;
  setTool: (tool: MapTool) => void;
  select: (objectId: string | null) => void;
  setPendingFocus: (focus: PendingFocus | null) => void;
  saveViewport: (mapEntityId: string, transform: Transform) => void;
  clearViewport: (mapEntityId: string) => void;
  setActiveLayer: (mapEntityId: string, layerId: string) => void;
  setPathPreset: (preset: PathPreset) => void;
  setPanelOpen: (open: boolean) => void;
  setLabelInput: (input: LabelInput | null) => void;
  setStampIcon: (icon: string) => void;
}

export const useMapToolStore = create<MapToolState>()((set) => ({
  tool: "select",
  selectedObjectId: null,
  pendingFocus: null,
  viewports: {},
  activeLayerIds: {},
  pathPreset: "river",
  panelOpen: true,
  labelInput: null,
  stampIcon: "mountain",
  // Switching tool also clears the selection: objects are only edited in Select.
  setTool: (tool) => set({ tool, selectedObjectId: null, labelInput: null }),
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
  setLabelInput: (labelInput) => set({ labelInput }),
  setStampIcon: (stampIcon) => set({ stampIcon }),
}));
