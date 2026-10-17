import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands } from "@/lib/bindings";
import { unwrap } from "@/lib/unwrap";

export const searchKeys = {
  /** Prefix shared by every search query, so one call invalidates them all. */
  all: ["search"] as const,
  query: (text: string, limit: number) => ["search", text, limit] as const,
};

/**
 * Full-text search. Disabled for blank input. `keepPreviousData` keeps the old
 * results on screen while the next keystroke's results load, so the list
 * doesn't flicker as you type.
 */
export function useSearch(text: string, limit = 20) {
  const trimmed = text.trim();
  return useQuery({
    queryKey: searchKeys.query(trimmed, limit),
    queryFn: () => unwrap(commands.search(trimmed, limit)),
    enabled: trimmed.length > 0,
    placeholderData: keepPreviousData,
  });
}

/** Recovery tool: rebuilds the whole search index from the data. */
export function useRebuildSearchIndex() {
  return useMutation({
    mutationFn: () => unwrap(commands.rebuildSearchIndex()),
  });
}

/** Dev builds only: creates N generated entities (the backend refuses in release). */
export function useSeedEntities() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (count: number) => unwrap(commands.devSeedEntities(count)),
    onSuccess: () =>
      Promise.all([
        // Query key literals avoid importing entities/queries (keeps imports one-way).
        queryClient.invalidateQueries({ queryKey: ["entities"] }),
        queryClient.invalidateQueries({ queryKey: searchKeys.all }),
      ]),
  });
}
