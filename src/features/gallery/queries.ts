import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { entityKeys } from "@/features/entities/queries";
import { commands, type EntityImage } from "@/lib/bindings";
import { unwrap } from "@/lib/unwrap";

export const galleryKeys = {
  forEntity: (entityId: string) => ["gallery", entityId] as const,
};

/** The images of one entity in gallery order. */
export function useEntityImages(entityId: string) {
  return useQuery({
    queryKey: galleryKeys.forEntity(entityId),
    queryFn: () => unwrap(commands.listEntityImages(entityId)),
  });
}

/**
 * A gallery change can change the portrait (shown in the entity header, the
 * sidebar and the graph) and bumps the entity's "last edited" time.
 */
export function invalidateGalleryData(queryClient: QueryClient, entityId: string) {
  return Promise.all([
    queryClient.invalidateQueries({ queryKey: galleryKeys.forEntity(entityId) }),
    queryClient.invalidateQueries({ queryKey: entityKeys.detail(entityId) }),
    queryClient.invalidateQueries({ queryKey: entityKeys.list }),
    queryClient.invalidateQueries({ queryKey: entityKeys.favorites }),
    queryClient.invalidateQueries({ queryKey: entityKeys.recent }),
    queryClient.invalidateQueries({ queryKey: entityKeys.portraits }),
  ]);
}

/**
 * Imports image files one after the other. If one fails, the ones before it
 * stay added and the error is thrown (so the caller can show it).
 */
export function useAddEntityImages(entityId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: async (paths: string[]) => {
      const added: EntityImage[] = [];
      for (const path of paths) added.push(await unwrap(commands.addEntityImage(entityId, path)));
      return added;
    },
    // Also on failure: earlier files of the batch were added.
    onSettled: () => invalidateGalleryData(queryClient, entityId),
  });
}

export function useUpdateImageCaption(entityId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: ({ id, caption }: { id: string; caption: string }) =>
      unwrap(commands.updateEntityImage(id, caption)),
    onSuccess: () => invalidateGalleryData(queryClient, entityId),
  });
}

/** `ids` lists every image of the entity once, in the new order. */
export function useReorderImages(entityId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (ids: string[]) => unwrap(commands.reorderEntityImages(entityId, ids)),
    onSuccess: (images: EntityImage[]) => {
      queryClient.setQueryData(galleryKeys.forEntity(entityId), images);
      void invalidateGalleryData(queryClient, entityId);
    },
  });
}

export function useRemoveImage(entityId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (id: string) => unwrap(commands.removeEntityImage(id)),
    onSuccess: () => invalidateGalleryData(queryClient, entityId),
  });
}

/** `imageId = null` clears the portrait. */
export function useSetPortrait(entityId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (imageId: string | null) => unwrap(commands.setPortrait(entityId, imageId)),
    onSuccess: (images: EntityImage[]) => {
      queryClient.setQueryData(galleryKeys.forEntity(entityId), images);
      void invalidateGalleryData(queryClient, entityId);
    },
  });
}
