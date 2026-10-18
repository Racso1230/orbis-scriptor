import { useCallback, useEffect, useMemo, type ReactNode } from "react";
import { useQueryClient } from "@tanstack/react-query";
import type { MapData } from "@/lib/bindings";
import { canRedo, canUndo, popRedo, popUndo, pushEntry, removeEntry } from "./command-stack";
import { ActionsContext, type MapActions } from "./map-actions-context";
import {
  getActiveHistory,
  newEntryId,
  runOps,
  setActiveHistory,
  setStack,
  stackOf,
  useMapHistoryStore,
  type Entry,
} from "./map-history";
import {
  buildCreate,
  buildDelete,
  buildUpdate,
  toNewObject,
  type MapAction,
  type NewObjectInput,
  type UpdateInput,
} from "./map-ops";
import { mapKeys } from "./queries";

interface ProviderProps {
  mapEntityId: string;
  onError: (message: string | null) => void;
  error: string | null;
  children: ReactNode;
}

/**
 * Gives every component under a map view the same undoable-actions API.
 * Every map edit (pins, shapes, labels, stamps, style, links) goes through
 * `create` / `update` / `remove` so it is undoable; layer operations are not.
 */
export function MapActionsProvider({ mapEntityId, onError, error, children }: ProviderProps) {
  const queryClient = useQueryClient();
  const stack = useMapHistoryStore((s) => s.stacks[mapEntityId]) ?? null;

  const getObjects = useCallback(
    () => queryClient.getQueryData<MapData>(mapKeys.detail(mapEntityId))?.objects ?? [],
    [queryClient, mapEntityId],
  );

  const send = useCallback(
    (ops: MapAction["do"], entryId: number) => {
      runOps(queryClient, mapEntityId, ops).catch((e: unknown) => {
        // The server refused: forget the entry so undo/redo cannot replay it.
        setStack(
          mapEntityId,
          removeEntry(stackOf(mapEntityId), (en) => en.id === entryId),
        );
        onError(e instanceof Error ? e.message : "The change could not be saved.");
      });
    },
    [queryClient, mapEntityId, onError],
  );

  const run = useCallback(
    (action: MapAction) => {
      if (action.do.length === 0) return;
      onError(null);
      const entry: Entry = { id: newEntryId(), action };
      setStack(mapEntityId, pushEntry(stackOf(mapEntityId), entry));
      send(action.do, entry.id);
    },
    [mapEntityId, send, onError],
  );

  const create = useCallback(
    (label: string, inputs: NewObjectInput[]) => {
      const objects = inputs.map(toNewObject);
      run(buildCreate(label, objects));
      return objects.map((o) => o.id ?? "");
    },
    [run],
  );

  const update = useCallback(
    (label: string, updates: UpdateInput[]) => {
      const action = buildUpdate(label, getObjects(), updates);
      if (action) run(action);
    },
    [run, getObjects],
  );

  const remove = useCallback(
    (label: string, ids: string[]) => {
      const action = buildDelete(label, getObjects(), ids);
      if (action) run(action);
    },
    [run, getObjects],
  );

  const undo = useCallback(() => {
    const { stack: next, entry } = popUndo(stackOf(mapEntityId));
    if (!entry) return;
    onError(null);
    setStack(mapEntityId, next);
    send(entry.action.undo, entry.id);
  }, [mapEntityId, send, onError]);

  const redo = useCallback(() => {
    const { stack: next, entry } = popRedo(stackOf(mapEntityId));
    if (!entry) return;
    onError(null);
    setStack(mapEntityId, next);
    send(entry.action.do, entry.id);
  }, [mapEntityId, send, onError]);

  useEffect(() => {
    const handlers = { undo, redo };
    setActiveHistory(handlers);
    return () => {
      if (getActiveHistory() === handlers) setActiveHistory(null);
    };
  }, [undo, redo]);

  const value = useMemo<MapActions>(
    () => ({
      create,
      update,
      remove,
      run,
      undo,
      redo,
      canUndo: stack ? canUndo(stack) : false,
      canRedo: stack ? canRedo(stack) : false,
      getObjects,
      error,
    }),
    [create, update, remove, run, undo, redo, stack, getObjects, error],
  );

  return <ActionsContext.Provider value={value}>{children}</ActionsContext.Provider>;
}
