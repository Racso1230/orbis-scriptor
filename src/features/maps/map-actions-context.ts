import { createContext, useContext } from "react";
import type { MapObject } from "@/lib/bindings";
import type { MapAction, NewObjectInput, UpdateInput } from "./map-ops";

// ---- the actions API ----------------------------------------------------

export interface MapActions {
  /** Create objects (one undo step). Returns the new ids in input order. */
  create: (label: string, inputs: NewObjectInput[]) => string[];
  /** Update objects (one undo step). Fields not in the patch are left alone. */
  update: (label: string, updates: UpdateInput[]) => void;
  /** Delete objects (one undo step). */
  remove: (label: string, ids: string[]) => void;
  /** Escape hatch: run a hand-built action. */
  run: (action: MapAction) => void;
  undo: () => void;
  redo: () => void;
  canUndo: boolean;
  canRedo: boolean;
  /** The objects of this map as the cache has them right now. */
  getObjects: () => MapObject[];
  /** Last error from a rejected action, for an inline message. */
  error: string | null;
}

export const ActionsContext = createContext<MapActions | null>(null);

export function useMapActions(): MapActions {
  const value = useContext(ActionsContext);
  if (!value) throw new Error("useMapActions must be used inside a MapActionsProvider");
  return value;
}
