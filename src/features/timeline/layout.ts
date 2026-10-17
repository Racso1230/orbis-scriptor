import type { EventSummary } from "@/lib/bindings";
import { assignLanes, eventExtent, type EventExtent } from "./lanes";
import { dayToX, type TimelineViewport } from "./scale";

export interface LaidOutEvent {
  event: EventSummary;
  /** Left edge of the marker/bar in pixels. */
  startX: number;
  /** Right edge of a bar in pixels; null for points. */
  endX: number | null;
  extent: EventExtent;
  lane: number;
}

/**
 * Pixel positions and lanes for a list of events at the current zoom. Lanes are
 * assigned over *all* loaded events (not just the visible ones), so an event
 * does not jump lane when its neighbour scrolls off screen.
 */
export function layoutEvents(
  events: readonly EventSummary[],
  viewport: TimelineViewport,
  width: number,
): { items: LaidOutEvent[]; laneCount: number } {
  const measured = events.map((event) => {
    const isRange = event.endDay !== null;
    // A range covers whole days: it ends at the *end* of its last day.
    const startX = dayToX(viewport, width, isRange ? event.startDay : event.startDay + 0.5);
    const endX = isRange ? dayToX(viewport, width, (event.endDay ?? event.startDay) + 1) : null;
    return { event, startX, endX, extent: eventExtent(startX, endX, event.name) };
  });

  const lanes = new Map(
    assignLanes(
      measured.map((m) => ({ id: m.event.entityId, start: m.extent.start, end: m.extent.end })),
    ).map((a) => [a.id, a.lane]),
  );
  const items = measured.map((m) => ({ ...m, lane: lanes.get(m.event.entityId) ?? 0 }));
  const laneCount = items.reduce((max, i) => Math.max(max, i.lane + 1), 0);
  return { items, laneCount };
}
