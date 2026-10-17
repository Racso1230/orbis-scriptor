import { create } from "zustand";
import { persist } from "zustand/middleware";

export type Theme = "light" | "dark" | "system";

interface UiState {
  theme: Theme;
  setTheme: (theme: Theme) => void;
}

/**
 * UI-only state (nothing here comes from the database).
 * `theme` is only a localStorage cache of the persisted app setting (for a
 * flash-free first paint); the source of truth is `features/settings`.
 */
export const useUiStore = create<UiState>()(
  persist(
    (set) => ({
      theme: "system",
      setTheme: (theme) => set({ theme }),
    }),
    { name: "orbis-ui" },
  ),
);
