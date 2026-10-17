import { beforeEach, describe, expect, it } from "vitest";
import { DEFAULT_VIEWPORT } from "./scale";
import { useTimelineStore } from "./timeline-store";

describe("timeline store", () => {
  beforeEach(() => {
    useTimelineStore.setState({ viewport: DEFAULT_VIEWPORT, pendingFocus: null });
  });

  it("remembers the viewport", () => {
    useTimelineStore.getState().saveViewport({ centerDay: 10, daysPerPixel: 0.5 });
    expect(useTimelineStore.getState().viewport).toEqual({ centerDay: 10, daysPerPixel: 0.5 });
  });

  it("holds a pending focus until it is cleared", () => {
    useTimelineStore.getState().setPendingFocus({ startDay: 5, endDay: 9 });
    expect(useTimelineStore.getState().pendingFocus).toEqual({ startDay: 5, endDay: 9 });
    useTimelineStore.getState().setPendingFocus(null);
    expect(useTimelineStore.getState().pendingFocus).toBeNull();
  });
});
