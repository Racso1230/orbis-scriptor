import type { QueryClient } from "@tanstack/react-query";
import { create } from "zustand";
import { commands, type MapData } from "@/lib/bindings";
import { unwrap } from "@/lib/unwrap";
import { emptyStack, type CommandStack } from "./command-stack";
import { applyOpsToData, type MapAction } from "./map-ops";
import { mapKeys } from "./queries";

// ---- history store ------------------------------------------------------

export interface Entry {
  id: number;
  action: MapAction;
}

interface HistoryState {
  /**
   * One stack per map, kept in memory only. Switching tabs keeps the history
   * of each map; closing the app (or the world) drops it.
   */
  stacks: Record<string, CommandStack<Entry>>;
}

export const useMapHistoryStore = create<HistoryState>()(() => ({ stacks: {} }));

let nextEntryId = 1;

export function newEntryId(): number {
  return nextEntryId++;
}

export function stackOf(mapEntityId: string): CommandStack<Entry> {
  return useMapHistoryStore.getState().stacks[mapEntityId] ?? emptyStack<Entry>();
}

export function setStack(mapEntityId: string, stack: CommandStack<Entry>) {
  useMapHistoryStore.setState((s) => ({ stacks: { ...s.stacks, [mapEntityId]: stack } }));
}

/** Forget a map's history (used when layer changes make old entries unusable). */
export function clearMapHistory(mapEntityId: string) {
  setStack(mapEntityId, emptyStack<Entry>());
}

// ---- executing op batches ----------------------------------------------

/** Per-map promise chain, so batches reach the backend in the order they were made. */
const queues = new Map<string, Promise<unknown>>();
const inFlight = new Map<string, number>();

/**
 * Applies `ops` to the cached map right away (optimistic) and sends them to
 * the backend as one atomic batch. Rejects if the backend refuses; the cache
 * is then refetched, which undoes the optimistic change.
 */
export async function runOps(
  queryClient: QueryClient,
  mapEntityId: string,
  ops: MapAction["do"],
): Promise<void> {
  const key = mapKeys.detail(mapEntityId);
  // Stop an in-flight refetch from overwriting the optimistic state.
  void queryClient.cancelQueries({ queryKey: key });
  queryClient.setQueryData<MapData>(key, (data) => (data ? applyOpsToData(data, ops) : data));

  inFlight.set(mapEntityId, (inFlight.get(mapEntityId) ?? 0) + 1);
  const previous = queues.get(mapEntityId) ?? Promise.resolve();
  const task = previous
    .catch(() => undefined)
    .then(() => unwrap(commands.applyMapOps(mapEntityId, ops)));
  queues.set(mapEntityId, task);
  try {
    await task;
  } finally {
    const left = (inFlight.get(mapEntityId) ?? 1) - 1;
    inFlight.set(mapEntityId, left);
    // Refetch once everything queued has settled: that replaces optimistic
    // placeholders with the server's rows (entity names etc.) and, after an
    // error, rolls the canvas back to what was really saved.
    if (left === 0) {
      void queryClient.invalidateQueries({ queryKey: key });
      void queryClient.invalidateQueries({ queryKey: mapKeys.appearances });
    }
  }
}

/** Registered by the mounted map view so global Ctrl+Z reaches it. */
let activeHistory: { undo: () => void; redo: () => void } | null = null;

export function setActiveHistory(h: { undo: () => void; redo: () => void } | null) {
  activeHistory = h;
}

export function getActiveHistory() {
  return activeHistory;
}

/** Called by the global shortcut handler. Returns true when a map handled it. */
export function triggerActiveMapHistory(kind: "undo" | "redo"): boolean {
  if (!activeHistory) return false;
  activeHistory[kind]();
  return true;
}
