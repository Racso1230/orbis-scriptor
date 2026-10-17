import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type Document } from "@/lib/bindings";
import { searchKeys } from "@/features/search/queries";
import { unwrap } from "@/lib/unwrap";

export const documentKeys = {
  detail: (entityId: string) => ["documents", entityId] as const,
};

export const backlinkKeys = {
  all: ["backlinks"] as const,
  detail: (entityId: string) => ["backlinks", entityId] as const,
};

/** Entities whose lore mentions this one. */
export function useBacklinks(entityId: string) {
  return useQuery({
    queryKey: backlinkKeys.detail(entityId),
    queryFn: () => unwrap(commands.getBacklinks(entityId)),
  });
}

export function useDocument(entityId: string) {
  return useQuery({
    queryKey: documentKeys.detail(entityId),
    queryFn: () => unwrap(commands.getDocument(entityId)),
    // The editor owns the content once loaded; never refetch underneath it.
    staleTime: Infinity,
  });
}

export function useSaveDocument() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      entityId,
      content,
      plainText,
    }: {
      entityId: string;
      content: string;
      plainText: string;
    }) => unwrap(commands.saveDocument(entityId, content, plainText)),
    onSuccess: (doc: Document) => {
      queryClient.setQueryData(documentKeys.detail(doc.entityId), doc);
      // Mentions may have changed, so every backlinks list is suspect.
      return Promise.all([
        queryClient.invalidateQueries({ queryKey: backlinkKeys.all }),
        // Lore text is indexed, so cached search results may be stale too.
        queryClient.invalidateQueries({ queryKey: searchKeys.all }),
      ]);
    },
  });
}
