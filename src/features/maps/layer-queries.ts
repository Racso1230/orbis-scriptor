import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { commands, type MapData, type MapLayer } from "@/lib/bindings";
import { unwrap } from "@/lib/unwrap";
import { clearMapHistory } from "./map-history";
import { mapKeys } from "./queries";

/**
 * Layer operations are NOT part of undo/redo: they are structural (deleting a
 * layer removes its objects for good) and are applied immediately. Deleting a
 * layer clears the map's undo history, because old entries may refer to
 * objects that no longer exist.
 */

function patchData(qc: QueryClient, mapEntityId: string, fn: (data: MapData) => MapData) {
  qc.setQueryData<MapData>(mapKeys.detail(mapEntityId), (data) => (data ? fn(data) : data));
}

function replaceLayer(qc: QueryClient, mapEntityId: string, layer: MapLayer) {
  patchData(qc, mapEntityId, (data) => ({
    ...data,
    layers: data.layers.map((l) => (l.id === layer.id ? layer : l)),
  }));
}

export function useCreateLayer(mapEntityId: string) {
  const qc = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (name: string) => unwrap(commands.createLayer(mapEntityId, name)),
    onSuccess: (layer) =>
      patchData(qc, mapEntityId, (data) => ({ ...data, layers: [...data.layers, layer] })),
  });
}

export function useRenameLayer(mapEntityId: string) {
  const qc = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: ({ layerId, name }: { layerId: string; name: string }) =>
      unwrap(commands.renameLayer(layerId, name)),
    onSuccess: (layer) => replaceLayer(qc, mapEntityId, layer),
  });
}

/** Visibility and lock are patched optimistically so the toggle feels instant. */
function useLayerFlag(
  mapEntityId: string,
  flag: "visible" | "locked",
  send: (layerId: string, value: boolean) => Promise<MapLayer>,
) {
  const qc = useQueryClient();
  const key = mapKeys.detail(mapEntityId);
  return useMutation({
    meta: { inlineError: true },
    mutationFn: ({ layerId, value }: { layerId: string; value: boolean }) => send(layerId, value),
    onMutate: ({ layerId, value }) => {
      const previous = qc.getQueryData<MapData>(key);
      patchData(qc, mapEntityId, (data) => ({
        ...data,
        layers: data.layers.map((l) => (l.id === layerId ? { ...l, [flag]: value } : l)),
      }));
      return { previous };
    },
    onError: (_e, _v, context) => {
      if (context?.previous) qc.setQueryData(key, context.previous);
    },
    onSuccess: (layer) => replaceLayer(qc, mapEntityId, layer),
  });
}

export function useSetLayerVisibility(mapEntityId: string) {
  return useLayerFlag(mapEntityId, "visible", (id, v) =>
    unwrap(commands.setLayerVisibility(id, v)),
  );
}

export function useSetLayerLocked(mapEntityId: string) {
  return useLayerFlag(mapEntityId, "locked", (id, v) => unwrap(commands.setLayerLocked(id, v)));
}

/** `layerIdsInOrder` is bottom first and must list every layer of the map. */
export function useReorderLayers(mapEntityId: string) {
  const qc = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (layerIdsInOrder: string[]) =>
      unwrap(commands.reorderLayers(mapEntityId, layerIdsInOrder)),
    onSuccess: (layers) => patchData(qc, mapEntityId, (data) => ({ ...data, layers })),
  });
}

export function useDeleteLayer(mapEntityId: string) {
  const qc = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (layerId: string) => unwrap(commands.deleteLayer(layerId)),
    onSuccess: (_void, layerId) => {
      clearMapHistory(mapEntityId);
      patchData(qc, mapEntityId, (data) => ({
        ...data,
        layers: data.layers.filter((l) => l.id !== layerId),
        objects: data.objects.filter((o) => o.layerId !== layerId),
      }));
      // Labels on other layers may have followed a path that was just deleted.
      void qc.invalidateQueries({ queryKey: mapKeys.detail(mapEntityId) });
      void qc.invalidateQueries({ queryKey: mapKeys.appearances });
    },
  });
}
