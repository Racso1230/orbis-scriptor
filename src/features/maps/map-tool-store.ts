import { create } from "zustand";
import type { Transform } from "./viewport";

export type MapTool = "select" | "pin";

/** Ask the map view to centre on a pin once it has loaded ("jump to" links). */
export interface PendingFocus {
  mapEntityId: string;
  objectId: string;
}

interface MapToolState {
  tool: MapTool;
  selectedObjectId: string | null;
  pendingFocus: PendingFocus | null;
  /** Last pan/zoom per map entity, so switching tabs keeps your place. */
  viewports: Record<string, Transform>;
  setTool: (tool: MapTool) => void;
  select: (objectId: string | null) => void;
  setPendingFocus: (focus: PendingFocus | null) => void;
  saveViewport: (mapEntityId: string, transform: Transform) => void;
  clearViewport: (mapEntityId: string) => void;
}

export const useMapToolStore = create<MapToolState>()((set) => ({
  tool: "select",
  selectedObjectId: null,
  pendingFocus: null,
  viewports: {},
  // Switching tool also clears the selection: pins are only editable in Select.
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
}));
