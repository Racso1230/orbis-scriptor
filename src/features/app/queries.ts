import { useQuery } from "@tanstack/react-query";
import { commands } from "@/lib/bindings";
import { unwrap } from "@/lib/unwrap";

export const appKeys = {
  info: ["app", "info"] as const,
};

/** Name and version of the running app, fetched once from Rust. */
export function useAppInfo() {
  return useQuery({
    queryKey: appKeys.info,
    queryFn: () => unwrap(commands.appInfo()),
    staleTime: Infinity,
  });
}
