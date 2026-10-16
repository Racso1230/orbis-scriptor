import { QueryClient } from "@tanstack/react-query";

/**
 * One QueryClient for the whole app. Desktop defaults: data only changes when
 * this app changes it, so don't refetch just because the window got focus.
 */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
      retry: false,
    },
  },
});
