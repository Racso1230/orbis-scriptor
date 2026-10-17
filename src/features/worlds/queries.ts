import { queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands } from "@/lib/bindings";
import { unwrap } from "@/lib/unwrap";

export const worldKeys = {
  current: ["worlds", "current"] as const,
  recent: ["worlds", "recent"] as const,
};

/** Exported as options so the router can `ensureQueryData` it in `beforeLoad`. */
export const currentWorldQuery = queryOptions({
  queryKey: worldKeys.current,
  queryFn: () => unwrap(commands.currentWorld()),
});

export function useCurrentWorld() {
  return useQuery(currentWorldQuery);
}

export function useRecentWorlds() {
  return useQuery({
    queryKey: worldKeys.recent,
    queryFn: () => unwrap(commands.listRecentWorlds()),
  });
}

/** Every world mutation changes both the current world and the recent list. */
function useInvalidateWorlds() {
  const queryClient = useQueryClient();
  return () =>
    Promise.all([
      queryClient.invalidateQueries({ queryKey: worldKeys.current }),
      queryClient.invalidateQueries({ queryKey: worldKeys.recent }),
    ]);
}

export function useCreateWorld() {
  const invalidate = useInvalidateWorlds();
  return useMutation({
    mutationFn: ({ path, name }: { path: string; name: string }) =>
      unwrap(commands.createWorld(path, name)),
    onSuccess: invalidate,
  });
}

/** Creates "The Shattered Coast" inside the chosen parent folder and opens it. */
export function useCreateSampleWorld() {
  const invalidate = useInvalidateWorlds();
  return useMutation({
    mutationFn: (parentDir: string) => unwrap(commands.createSampleWorld(parentDir)),
    onSuccess: invalidate,
  });
}

export function useOpenWorld() {
  const invalidate = useInvalidateWorlds();
  return useMutation({
    mutationFn: (path: string) => unwrap(commands.openWorld(path)),
    onSuccess: invalidate,
  });
}

export function useCloseWorld() {
  const invalidate = useInvalidateWorlds();
  return useMutation({
    mutationFn: () => unwrap(commands.closeWorld()),
    onSuccess: invalidate,
  });
}

export function useRemoveRecentWorld() {
  const invalidate = useInvalidateWorlds();
  return useMutation({
    mutationFn: (path: string) => unwrap(commands.removeRecentWorld(path)),
    onSuccess: invalidate,
  });
}
