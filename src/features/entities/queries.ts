import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type Entity, type EntityPatch, type NewEntity } from "@/lib/bindings";
import { searchKeys } from "@/features/search/queries";
import { unwrap } from "@/lib/unwrap";

export const entityKeys = {
  types: ["entities", "types"] as const,
  list: ["entities", "list"] as const,
  detail: (id: string) => ["entities", "detail", id] as const,
  favorites: ["entities", "favorites"] as const,
  recent: ["entities", "recent"] as const,
  portraits: ["entities", "portraits"] as const,
};

/** How many entities the sidebar's "Recently edited" section shows. */
export const RECENT_LIMIT = 8;

/** Built-in entity types with their field definitions (static for the session). */
export function useEntityTypes() {
  return useQuery({
    queryKey: entityKeys.types,
    queryFn: () => unwrap(commands.entityTypes()),
    staleTime: Infinity,
  });
}

/** All entities of the open world (summaries, for the sidebar and tabs). */
export function useEntities() {
  return useQuery({
    queryKey: entityKeys.list,
    queryFn: () => unwrap(commands.listEntities(null)),
  });
}

/** Starred entities (by type, then name). */
export function useFavorites() {
  return useQuery({
    queryKey: entityKeys.favorites,
    queryFn: () => unwrap(commands.listFavorites()),
  });
}

/** The most recently changed entities, newest first. */
export function useRecent(limit = RECENT_LIMIT) {
  return useQuery({
    queryKey: [...entityKeys.recent, limit] as const,
    queryFn: () => unwrap(commands.listRecent(limit)),
  });
}

/** Portraits of every entity that has one (for graph nodes and other avatars). */
export function usePortraits() {
  return useQuery({
    queryKey: entityKeys.portraits,
    queryFn: () => unwrap(commands.listPortraits()),
  });
}

/** The portrait of one entity (or `null`), read from the shared portraits list. */
export function usePortrait(entityId: string) {
  return useQuery({
    queryKey: entityKeys.portraits,
    queryFn: () => unwrap(commands.listPortraits()),
    select: (list) => list.find((p) => p.entityId === entityId)?.portrait ?? null,
  });
}

/** Stars or un-stars an entity. Does not change its "last edited" time. */
export function useSetFavorite() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, favorite }: { id: string; favorite: boolean }) =>
      unwrap(commands.setFavorite(id, favorite)),
    onSuccess: (_result, { id, favorite }) => {
      queryClient.setQueryData<Entity>(entityKeys.detail(id), (old) =>
        old ? { ...old, favorite } : old,
      );
      return Promise.all([
        queryClient.invalidateQueries({ queryKey: entityKeys.list }),
        queryClient.invalidateQueries({ queryKey: entityKeys.favorites }),
        queryClient.invalidateQueries({ queryKey: entityKeys.recent }),
      ]);
    },
  });
}

export function useEntity(id: string) {
  return useQuery({
    queryKey: entityKeys.detail(id),
    queryFn: () => unwrap(commands.getEntity(id)),
  });
}

export function useCreateEntity() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: NewEntity) => unwrap(commands.createEntity(input)),
    onSuccess: (entity: Entity) => {
      queryClient.setQueryData(entityKeys.detail(entity.id), entity);
      return Promise.all([
        queryClient.invalidateQueries({ queryKey: entityKeys.list }),
        queryClient.invalidateQueries({ queryKey: entityKeys.recent }),
        queryClient.invalidateQueries({ queryKey: searchKeys.all }),
        // A new `event` entity appears on the timeline (at day 0).
        queryClient.invalidateQueries({ queryKey: ["events"] }),
      ]);
    },
  });
}

export function useUpdateEntity() {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: ({ id, patch }: { id: string; patch: EntityPatch }) =>
      unwrap(commands.updateEntity(id, patch)),
    onSuccess: (entity: Entity) => {
      queryClient.setQueryData(entityKeys.detail(entity.id), entity);
      // Not awaited: autosave shows "Saved" once the save itself is done, not
      // after every open list, map and graph has refetched (a slow or
      // cancelled refetch used to leave the indicator stuck on "Saving…").
      void Promise.all([
        queryClient.invalidateQueries({ queryKey: entityKeys.list }),
        queryClient.invalidateQueries({ queryKey: entityKeys.favorites }),
        queryClient.invalidateQueries({ queryKey: entityKeys.recent }),
        queryClient.invalidateQueries({ queryKey: searchKeys.all }),
        // Pins show the linked entity's name and type.
        queryClient.invalidateQueries({ queryKey: ["maps"] }),
        // Timeline events show the entity name and vanish with the entity.
        queryClient.invalidateQueries({ queryKey: ["events"] }),
        // Relationship lists and graphs show the entity name and vanish with it.
        queryClient.invalidateQueries({ queryKey: ["relationships"] }),
        queryClient.invalidateQueries({ queryKey: ["graph"] }),
      ]);
    },
  });
}

export function useDeleteEntity() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.deleteEntity(id)),
    onSuccess: (_result, id) => {
      queryClient.removeQueries({ queryKey: entityKeys.detail(id) });
      return Promise.all([
        queryClient.invalidateQueries({ queryKey: entityKeys.list }),
        queryClient.invalidateQueries({ queryKey: entityKeys.favorites }),
        queryClient.invalidateQueries({ queryKey: entityKeys.recent }),
        queryClient.invalidateQueries({ queryKey: entityKeys.portraits }),
        queryClient.invalidateQueries({ queryKey: ["tags"] }),
        queryClient.invalidateQueries({ queryKey: searchKeys.all }),
        // Pins on maps show the linked entity's name and type.
        queryClient.invalidateQueries({ queryKey: ["maps"] }),
        // Timeline events show the entity name and vanish with the entity.
        queryClient.invalidateQueries({ queryKey: ["events"] }),
        // Relationship lists and graphs show the entity name and vanish with it.
        queryClient.invalidateQueries({ queryKey: ["relationships"] }),
        queryClient.invalidateQueries({ queryKey: ["graph"] }),
      ]);
    },
  });
}
