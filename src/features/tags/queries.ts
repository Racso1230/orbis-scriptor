import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { entityKeys } from "@/features/entities/queries";
import { searchKeys } from "@/features/search/queries";
import { commands, type Entity } from "@/lib/bindings";
import { unwrap } from "@/lib/unwrap";

export const tagKeys = {
  all: ["tags"] as const,
  list: ["tags", "list"] as const,
  byTag: (names: string[], matchAll: boolean) => ["tags", "entities", names, matchAll] as const,
};

/** Every tag of the world with its colour and entity count. */
export function useTags() {
  return useQuery({
    queryKey: tagKeys.list,
    queryFn: () => unwrap(commands.listTags()),
  });
}

/** Entities carrying the given tags (nested tags count as their parent). */
export function useEntitiesByTag(names: string[], matchAll = false) {
  return useQuery({
    queryKey: tagKeys.byTag(names, matchAll),
    queryFn: () => unwrap(commands.listEntitiesByTag(names, matchAll)),
    enabled: names.length > 0,
  });
}

/** Tags show up in the sidebar, search results and the Markdown export. Not awaited by callers. */
export function invalidateTagData(queryClient: QueryClient) {
  return Promise.all([
    queryClient.invalidateQueries({ queryKey: tagKeys.all }),
    queryClient.invalidateQueries({ queryKey: entityKeys.list }),
    queryClient.invalidateQueries({ queryKey: entityKeys.favorites }),
    queryClient.invalidateQueries({ queryKey: entityKeys.recent }),
    queryClient.invalidateQueries({ queryKey: searchKeys.all }),
  ]);
}

/** Replaces the tags of one entity (missing tags are created). */
export function useSetEntityTags(entityId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (names: string[]) => unwrap(commands.setEntityTags(entityId, names)),
    onSuccess: (tags: string[]) => {
      queryClient.setQueryData<Entity>(entityKeys.detail(entityId), (old) =>
        old ? { ...old, tags } : old,
      );
      void invalidateTagData(queryClient);
    },
  });
}

export function useRenameTag() {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: ({ id, name }: { id: string; name: string }) =>
      unwrap(commands.renameTag(id, name)),
    onSuccess: () => {
      // Open entity pages show their tags, so refresh the details as well.
      void queryClient.invalidateQueries({ queryKey: ["entities", "detail"] });
      return invalidateTagData(queryClient);
    },
  });
}

export function useDeleteTag() {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (id: string) => unwrap(commands.deleteTag(id)),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["entities", "detail"] });
      return invalidateTagData(queryClient);
    },
  });
}

export function useSetTagColor() {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: ({ id, color }: { id: string; color: string | null }) =>
      unwrap(commands.setTagColor(id, color)),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: tagKeys.all }),
  });
}
