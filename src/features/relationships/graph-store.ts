import { create } from "zustand";

export type GraphMode = "graph" | "family";

interface GraphState {
  focusId: string | null;
  /** Previously focused entities, most recent last (for the Back button). */
  history: string[];
  depth: number;
  mode: GraphMode;
  hiddenKinds: string[];
  hiddenTypes: string[];
  includeMentions: boolean;
  includeInvolved: boolean;
  /** Focus an entity; remembers the previous focus unless `remember` is false. */
  setFocus: (id: string, remember?: boolean) => void;
  back: () => void;
  setDepth: (depth: number) => void;
  setMode: (mode: GraphMode) => void;
  toggleKinds: (keys: string[], visible: boolean) => void;
  toggleType: (key: string, visible: boolean) => void;
  setIncludeMentions: (on: boolean) => void;
  setIncludeInvolved: (on: boolean) => void;
}

const without = (list: string[], keys: string[]) => list.filter((k) => !keys.includes(k));

export const useGraphStore = create<GraphState>()((set, get) => ({
  focusId: null,
  history: [],
  depth: 2,
  mode: "graph",
  hiddenKinds: [],
  hiddenTypes: [],
  includeMentions: false,
  includeInvolved: false,

  setFocus: (id, remember = true) => {
    const { focusId, history } = get();
    if (id === focusId) return;
    set({
      focusId: id,
      history: remember && focusId ? [...history.slice(-19), focusId] : history,
    });
  },
  back: () => {
    const { history } = get();
    const previous = history[history.length - 1];
    if (previous) set({ focusId: previous, history: history.slice(0, -1) });
  },
  setDepth: (depth) => set({ depth: Math.min(4, Math.max(1, depth)) }),
  setMode: (mode) => set({ mode }),
  // `visible: false` hides the kinds (adds them to the hidden list).
  toggleKinds: (keys, visible) =>
    set((s) => ({
      hiddenKinds: visible
        ? without(s.hiddenKinds, keys)
        : [...without(s.hiddenKinds, keys), ...keys],
    })),
  toggleType: (key, visible) =>
    set((s) => ({
      hiddenTypes: visible
        ? without(s.hiddenTypes, [key])
        : [...without(s.hiddenTypes, [key]), key],
    })),
  setIncludeMentions: (includeMentions) => set({ includeMentions }),
  setIncludeInvolved: (includeInvolved) => set({ includeInvolved }),
}));
