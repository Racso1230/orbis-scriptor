import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { entityKeys } from "@/features/entities/queries";
import { searchKeys } from "@/features/search/queries";
import { e2eDir } from "@/lib/e2e";
import { open } from "@tauri-apps/plugin-dialog";
import {
  commands,
  type MapBackground,
  type MapData,
  type MapObject,
  type MapObjectPatch,
  type NewMapObject,
} from "@/lib/bindings";
import { unwrap } from "@/lib/unwrap";

export const mapKeys = {
  detail: (mapEntityId: string) => ["maps", "detail", mapEntityId] as const,
  /** Prefix for every "appears on maps" list, so one call invalidates them all. */
  appearances: ["maps", "appearances"] as const,
  appearancesFor: (entityId: string) => ["maps", "appearances", entityId] as const,
};

export function useMap(mapEntityId: string) {
  return useQuery({
    queryKey: mapKeys.detail(mapEntityId),
    queryFn: () => unwrap(commands.getMap(mapEntityId)),
  });
}

export function useEntityMapAppearances(entityId: string) {
  return useQuery({
    queryKey: mapKeys.appearancesFor(entityId),
    queryFn: () => unwrap(commands.listEntityMapAppearances(entityId)),
  });
}

/** Native file picker limited to common raster formats. `null` if cancelled. */
export async function pickImageFile(): Promise<string | null> {
  // Test hook: skip the native dialog (debug builds with ORBIS_E2E_DIR only).
  const hooked = await e2eDir();
  if (hooked) return `${hooked}/map.png`;
  const result = await open({
    multiple: false,
    directory: false,
    title: "Choose a map image",
    filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp"] }],
  });
  return typeof result === "string" ? result : null;
}

/** Imports an image file as an asset, then makes it the map's background. */
export function useSetMapBackground(mapEntityId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: async (path: string) => {
      const asset = await unwrap(commands.importAsset(path));
      return unwrap(commands.setMapBackground(mapEntityId, asset.id));
    },
    onSuccess: () => queryClient.invalidateQueries({ queryKey: mapKeys.detail(mapEntityId) }),
  });
}

export function useCreateMapObject(mapEntityId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (input: NewMapObject) => unwrap(commands.createMapObject(input)),
    onSuccess: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: mapKeys.detail(mapEntityId) }),
        queryClient.invalidateQueries({ queryKey: mapKeys.appearances }),
      ]),
  });
}

/**
 * Updates a map object. The cache is patched optimistically so a dragged pin
 * does not snap back while the save is in flight; on failure we roll back.
 */
export function useUpdateMapObject(mapEntityId: string) {
  const queryClient = useQueryClient();
  const key = mapKeys.detail(mapEntityId);
  return useMutation({
    meta: { inlineError: true },
    mutationFn: ({ id, patch }: { id: string; patch: MapObjectPatch }) =>
      unwrap(commands.updateMapObject(id, patch)),
    onMutate: async ({ id, patch }) => {
      await queryClient.cancelQueries({ queryKey: key });
      const previous = queryClient.getQueryData<MapData>(key);
      if (previous && patch.geometry) {
        const geometry = patch.geometry;
        queryClient.setQueryData<MapData>(key, {
          ...previous,
          objects: previous.objects.map((o) => (o.id === id ? { ...o, geometry } : o)),
        });
      }
      return { previous };
    },
    onError: (_error, _vars, context) => {
      if (context?.previous) queryClient.setQueryData(key, context.previous);
    },
    onSettled: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: key }),
        queryClient.invalidateQueries({ queryKey: mapKeys.appearances }),
      ]),
  });
}

export function useDeleteMapObject(mapEntityId: string) {
  const queryClient = useQueryClient();
  const key = mapKeys.detail(mapEntityId);
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (id: string) => unwrap(commands.deleteMapObject(id)),
    onMutate: async (id) => {
      await queryClient.cancelQueries({ queryKey: key });
      const previous = queryClient.getQueryData<MapData>(key);
      if (previous) {
        queryClient.setQueryData<MapData>(key, {
          ...previous,
          objects: previous.objects.filter((o: MapObject) => o.id !== id),
        });
      }
      return { previous };
    },
    onError: (_error, _id, context) => {
      if (context?.previous) queryClient.setQueryData(key, context.previous);
    },
    onSettled: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: key }),
        queryClient.invalidateQueries({ queryKey: mapKeys.appearances }),
      ]),
  });
}

/** Dev builds only: adds N random pins to the map. */
export function useSeedMapPins(mapEntityId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (count: number) => unwrap(commands.devSeedMapPins(mapEntityId, count)),
    onSuccess: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: mapKeys.detail(mapEntityId) }),
        queryClient.invalidateQueries({ queryKey: mapKeys.appearances }),
      ]),
  });
}

/** Creates a new blank map entity (name, size and background in one transaction). */
export function useCreateBlankMap() {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (input: {
      name: string;
      width: number;
      height: number;
      background: MapBackground;
    }) => unwrap(commands.createBlankMap(input.name, input.width, input.height, input.background)),
    onSuccess: (data) => {
      queryClient.setQueryData(mapKeys.detail(data.map.entityId), data);
      // The sidebar and palette list entities; make them pick up the new map.
      return Promise.all([
        queryClient.invalidateQueries({ queryKey: entityKeys.list }),
        queryClient.invalidateQueries({ queryKey: searchKeys.all }),
      ]);
    },
  });
}

/**
 * Turns an (empty) map entity into a blank map, or changes a blank map's size
 * and look. Only the parts that are given are sent; image maps are refused by
 * the backend.
 */
export function useConfigureBlankMap(mapEntityId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: async (input: {
      size?: { width: number; height: number };
      background?: MapBackground;
    }) => {
      if (input.size)
        await unwrap(commands.setMapSize(mapEntityId, input.size.width, input.size.height));
      if (input.background) {
        await unwrap(commands.setMapBackgroundStyle(mapEntityId, input.background));
      }
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: mapKeys.detail(mapEntityId) }),
  });
}
