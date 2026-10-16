import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type Document } from "@/lib/bindings";
import { unwrap } from "@/lib/unwrap";

export const documentKeys = {
  detail: (entityId: string) => ["documents", entityId] as const,
};

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
    },
  });
}
