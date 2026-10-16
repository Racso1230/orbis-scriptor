import { create } from "zustand";
import { persist } from "zustand/middleware";

export type Theme = "light" | "dark" | "system";

interface UiState {
  theme: Theme;
  setTheme: (theme: Theme) => void;
}

/**
 * UI-only state (nothing here comes from the database).
 * Persisted to localStorage for now; Phase 8 moves theme into app settings.
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
