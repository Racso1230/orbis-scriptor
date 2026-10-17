import { MutationCache, QueryCache, QueryClient } from "@tanstack/react-query";
import { toastError } from "@/lib/notify";

/**
 * Setting `meta: { inlineError: true }` on a query or mutation means the UI
 * already shows its failure inline (a form message, the save indicator...), so
 * the global handler below must not also show a toast.
 */
declare module "@tanstack/react-query" {
  interface Register {
    queryMeta: { inlineError?: boolean };
    mutationMeta: { inlineError?: boolean };
  }
}

/** Query keys whose failure was already toasted; cleared when the query succeeds again. */
const reportedQueries = new Set<string>();

/**
 * Builds the QueryClient. Desktop defaults: data only changes when this app
 * changes it, so don't refetch just because the window got focus.
 *
 * Error policy (one place for the whole app):
 * - Mutations toast their error unless `meta.inlineError` is set.
 * - Queries only toast when a *background refetch* fails (we already had data),
 *   and only once until the query succeeds again. A failed first load is shown
 *   by the component itself (it has nothing else to render).
 */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    queryCache: new QueryCache({
      onError: (error, query) => {
        if (query.meta?.inlineError || query.state.data === undefined) return;
        if (reportedQueries.has(query.queryHash)) return;
        reportedQueries.add(query.queryHash);
        toastError(error, `query-${query.queryHash}`);
      },
      onSuccess: (_data, query) => {
        reportedQueries.delete(query.queryHash);
      },
    }),
    mutationCache: new MutationCache({
      onError: (error, _variables, _context, mutation) => {
        if (mutation.meta?.inlineError) return;
        toastError(error);
      },
    }),
    defaultOptions: {
      queries: {
        refetchOnWindowFocus: false,
        retry: false,
      },
    },
  });
}

export const queryClient = createQueryClient();
