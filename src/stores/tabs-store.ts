import { create } from "zustand";
import { persist } from "zustand/middleware";

/** Open entity tabs, remembered per world (keyed by the world folder path). */
interface TabsState {
  byWorld: Record<string, string[]>;
  openTab: (world: string, entityId: string) => void;
  /** Closes a tab and returns the neighbour that should become active, if any. */
  closeTab: (world: string, entityId: string) => string | null;
}

export const useTabsStore = create<TabsState>()(
  persist(
    (set, get) => ({
      byWorld: {},

      openTab: (world, entityId) => {
        const tabs = get().byWorld[world] ?? [];
        if (tabs.includes(entityId)) return;
        set({ byWorld: { ...get().byWorld, [world]: [...tabs, entityId] } });
      },

      closeTab: (world, entityId) => {
        const tabs = get().byWorld[world] ?? [];
        const index = tabs.indexOf(entityId);
        if (index === -1) return null;
        const next = tabs.filter((id) => id !== entityId);
        set({ byWorld: { ...get().byWorld, [world]: next } });
        // Prefer the tab to the right, then the one to the left.
        return next[index] ?? next[index - 1] ?? null;
      },
    }),
    { name: "orbis-tabs" },
  ),
);

/** Tabs of one world (stable empty array when none). */
const NO_TABS: string[] = [];
export function useWorldTabs(world: string | undefined): string[] {
  return useTabsStore((s) => (world ? (s.byWorld[world] ?? NO_TABS) : NO_TABS));
}
