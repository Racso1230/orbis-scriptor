import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { commands, type ImportOptions } from "@/lib/bindings";
import { unwrap } from "@/lib/unwrap";

/**
 * What an import would do, without writing anything. Re-runs when the options
 * change. The scan reads the whole vault, so nothing is cached for long.
 */
export function useImportPreview(vaultDir: string | null, options: ImportOptions) {
  return useQuery({
    queryKey: ["import", "preview", vaultDir, options] as const,
    queryFn: () => unwrap(commands.previewImport(vaultDir ?? "", options)),
    enabled: vaultDir !== null,
    placeholderData: keepPreviousData,
    staleTime: 0,
    gcTime: 30_000,
    meta: { inlineError: true },
  });
}
