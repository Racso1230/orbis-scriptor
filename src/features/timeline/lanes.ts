/** Horizontal extent of one event in pixels, including its label. */
export interface LaneItem {
  id: string;
  start: number;
  end: number;
}

export interface LaneAssignment {
  id: string;
  lane: number;
}

/**
 * Greedy lane stacking: walk events in start order and put each in the lowest
 * lane whose last item ended before this one starts (plus `gap` pixels).
 * Returns assignments in the same order as the input after sorting by start
 * (ties keep the input order, so the result is stable).
 */
export function assignLanes(items: readonly LaneItem[], gap = 6): LaneAssignment[] {
  const sorted = items
    .map((item, index) => ({ item, index }))
    .sort((a, b) => a.item.start - b.item.start || a.index - b.index);

  const laneEnds: number[] = [];
  return sorted.map(({ item }) => {
    let lane = laneEnds.findIndex((end) => end + gap <= item.start);
    if (lane === -1) {
      lane = laneEnds.length;
      laneEnds.push(item.end);
    } else {
      laneEnds[lane] = item.end;
    }
    return { id: item.id, lane };
  });
}

export const POINT_RADIUS = 6;
/** Rough width of one label character at the 12px label size. */
export const CHAR_WIDTH = 6.6;
const LABEL_GAP = 6;

export interface EventExtent {
  start: number;
  end: number;
  /** For bars: whether the label fits inside the bar (otherwise it is drawn after it). */
  labelInside: boolean;
  labelWidth: number;
}

export function labelWidthOf(name: string): number {
  return name.length * CHAR_WIDTH;
}

/**
 * Pixel extent of an event. Points reserve room for their marker and label;
 * bars reserve their length, plus the label when it does not fit inside.
 * `startX`/`endX` are pixel positions (`endX` null for a point).
 */
export function eventExtent(startX: number, endX: number | null, name: string): EventExtent {
  const labelWidth = labelWidthOf(name);
  if (endX === null) {
    return {
      start: startX - POINT_RADIUS,
      end: startX + POINT_RADIUS + LABEL_GAP + labelWidth,
      labelInside: false,
      labelWidth,
    };
  }
  const width = Math.max(endX - startX, POINT_RADIUS * 2);
  const labelInside = width >= labelWidth + 12;
  return {
    start: startX,
    end: labelInside ? startX + width : startX + width + LABEL_GAP + labelWidth,
    labelInside,
    labelWidth,
  };
}
