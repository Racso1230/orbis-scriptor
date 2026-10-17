import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type AppSettings } from "@/lib/bindings";
import { unwrap } from "@/lib/unwrap";

/** Settings with every field present (the generated type marks them optional). */
export type Settings = Required<AppSettings>;

/** Same defaults as the Rust side; used until the real settings have loaded. */
export const DEFAULT_SETTINGS: Settings = {
  theme: "system",
  autosaveDelayMs: 800,
  editorFontSize: 16,
};

export const settingsKeys = {
  all: ["settings"] as const,
};

function withDefaults(settings: AppSettings): Settings {
  return { ...DEFAULT_SETTINGS, ...settings };
}

export function useSettings() {
  return useQuery({
    queryKey: settingsKeys.all,
    queryFn: async () => withDefaults(await unwrap(commands.getSettings())),
    staleTime: Infinity,
  });
}

/** The settings, or the defaults while they are still loading. */
export function useSettingsValue(): Settings {
  return useSettings().data ?? DEFAULT_SETTINGS;
}

/**
 * Saves new settings. The cache is updated immediately (optimistically) so the
 * UI reacts at once, then replaced by the clamped values Rust sends back.
 */
export function useUpdateSettings() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (settings: Settings) =>
      withDefaults(await unwrap(commands.updateSettings(settings))),
    onMutate: (settings) => {
      queryClient.setQueryData(settingsKeys.all, settings);
    },
    onSuccess: (saved) => {
      queryClient.setQueryData(settingsKeys.all, saved);
    },
    onError: () => queryClient.invalidateQueries({ queryKey: settingsKeys.all }),
  });
}
