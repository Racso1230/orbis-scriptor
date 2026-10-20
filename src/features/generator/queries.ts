import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { commands, type GeneratorParams } from "@/lib/bindings";
import { unwrap } from "@/lib/unwrap";
import { PREVIEW_SIDE } from "./wizard-model";

/**
 * A low-resolution preview of the terrain as a `data:` URL. Pass `null` to
 * pause (invalid settings). The caller debounces `params`; the previous image
 * stays visible while the next one renders. Previews are deterministic, so a
 * result never goes stale.
 */
export function usePreviewGeneration(params: GeneratorParams | null, maxSide = PREVIEW_SIDE) {
  return useQuery({
    queryKey: ["generator", "preview", params, maxSide] as const,
    queryFn: async () => {
      const base64 = await unwrap(commands.previewGeneration(params as GeneratorParams, maxSide));
      return `data:image/png;base64,${base64}`;
    },
    enabled: params !== null,
    placeholderData: keepPreviousData,
    staleTime: Infinity,
    gcTime: 60_000,
    meta: { inlineError: true },
  });
}
