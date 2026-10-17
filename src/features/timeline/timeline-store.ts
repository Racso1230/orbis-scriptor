import { create } from "zustand";
import { DEFAULT_VIEWPORT, type TimelineViewport } from "./scale";

/** Ask the timeline to frame a day range once it knows its size ("Show on timeline"). */
export interface TimelineFocus {
  startDay: number;
  endDay: number;
}

interface TimelineState {
  /** Last pan/zoom, so leaving and returning to the timeline keeps your place. */
  viewport: TimelineViewport;
  pendingFocus: TimelineFocus | null;
  saveViewport: (viewport: TimelineViewport) => void;
  setPendingFocus: (focus: TimelineFocus | null) => void;
}

export const useTimelineStore = create<TimelineState>()((set) => ({
  viewport: DEFAULT_VIEWPORT,
  pendingFocus: null,
  saveViewport: (viewport) => set({ viewport }),
  setPendingFocus: (pendingFocus) => set({ pendingFocus }),
}));
