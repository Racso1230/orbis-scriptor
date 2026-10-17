/**
 * Timeline viewport: which day is in the middle of the screen and how many
 * days one pixel covers. A day `d` appears at `width / 2 + (d - centerDay) / daysPerPixel`.
 */
export interface TimelineViewport {
  centerDay: number;
  daysPerPixel: number;
}

/** About 3 days across a 1000px timeline. */
export const MIN_DAYS_PER_PIXEL = 0.003;
/** About 200,000 days (~550 years of 360 days) across a 1000px timeline. */
export const MAX_DAYS_PER_PIXEL = 200;
/** Each wheel notch multiplies (or divides) the scale by this. */
export const ZOOM_STEP = 1.2;
export const DEFAULT_VIEWPORT: TimelineViewport = { centerDay: 0, daysPerPixel: 3 };

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export function clampScale(daysPerPixel: number): number {
  return clamp(daysPerPixel, MIN_DAYS_PER_PIXEL, MAX_DAYS_PER_PIXEL);
}

export function dayToX(v: TimelineViewport, width: number, day: number): number {
  return width / 2 + (day - v.centerDay) / v.daysPerPixel;
}

export function xToDay(v: TimelineViewport, width: number, x: number): number {
  return v.centerDay + (x - width / 2) * v.daysPerPixel;
}

/** Fractional day range currently visible. */
export function visibleRange(v: TimelineViewport, width: number): { from: number; to: number } {
  const half = (width / 2) * v.daysPerPixel;
  return { from: v.centerDay - half, to: v.centerDay + half };
}

/** Drag by `dxPixels` to the right: the world follows the cursor, so the centre moves left. */
export function panBy(v: TimelineViewport, dxPixels: number): TimelineViewport {
  return { ...v, centerDay: v.centerDay - dxPixels * v.daysPerPixel };
}

/**
 * Zoom one wheel step around the pointer so the day under the cursor stays
 * under the cursor. Negative `deltaY` (wheel up) zooms in.
 */
export function zoomAround(
  v: TimelineViewport,
  width: number,
  pointerX: number,
  deltaY: number,
): TimelineViewport {
  if (deltaY === 0) return v;
  const target = deltaY < 0 ? v.daysPerPixel / ZOOM_STEP : v.daysPerPixel * ZOOM_STEP;
  const daysPerPixel = clampScale(target);
  const anchorDay = xToDay(v, width, pointerX);
  // Solve anchorDay = centre' + (pointerX - width/2) * scale' for the new centre.
  return { daysPerPixel, centerDay: anchorDay - (pointerX - width / 2) * daysPerPixel };
}

/** Viewport that shows `[startDay, endDay + 1)` with some padding in pixels. */
export function fitRange(
  startDay: number,
  endDay: number,
  width: number,
  paddingPx = 80,
): TimelineViewport {
  const usable = Math.max(1, width - paddingPx * 2);
  const span = Math.max(1, endDay + 1 - startDay);
  return {
    centerDay: (startDay + endDay + 1) / 2,
    daysPerPixel: clampScale(span / usable),
  };
}

/**
 * The day window to fetch for a viewport. It is padded by one screen on each
 * side and snapped to a power-of-two grid, so panning a little (or zooming
 * within the same size class) keeps producing the same window - and therefore
 * the same query key - instead of refetching on every frame.
 */
export function queryWindow(from: number, to: number): { from: number; to: number } {
  const span = Math.max(1, to - from);
  const cell = Math.pow(2, Math.max(4, Math.ceil(Math.log2(span))));
  return {
    from: Math.floor((from - cell) / cell) * cell,
    to: Math.ceil((to + cell) / cell) * cell,
  };
}

/**
 * Viewport that frames one event for "Show on timeline": the event takes up
 * about 60% of the width, but a point event still shows a month of context.
 */
export function focusViewport(startDay: number, endDay: number, width: number): TimelineViewport {
  const span = Math.max(endDay + 1 - startDay, 30);
  return {
    centerDay: (startDay + endDay + 1) / 2,
    daysPerPixel: clampScale(span / Math.max(1, width * 0.6)),
  };
}
