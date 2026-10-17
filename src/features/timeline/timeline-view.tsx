import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { FlaskConical, Maximize2, Minus, Plus, RotateCcw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useOpenEntity } from "@/features/entities/use-open-entity";
import { useIndexMaintenance } from "@/features/search/use-index-maintenance";
import type { CalendarDefinition, EventSummary } from "@/lib/bindings";
import { formatDate } from "./calendar";
import { CreateEventDialog } from "./create-event-dialog";
import { layoutEvents } from "./layout";
import {
  DEFAULT_VIEWPORT,
  clamp,
  dayToX,
  fitRange,
  focusViewport,
  panBy,
  queryWindow,
  visibleRange,
  xToDay,
  zoomAround,
  type TimelineViewport,
} from "./scale";
import { generateTicks } from "./ticks";
import { useTimelineStore } from "./timeline-store";
import { useCreateEvent, useDefaultCalendar, useEvents, useFetchAllEvents } from "./queries";

const AXIS_HEIGHT = 36;
const LANES_TOP = 56;
const LANE_HEIGHT = 30;
/** Pointer must move this far before a press becomes a drag (so clicks still click). */
const DRAG_THRESHOLD = 4;
const FALLBACK_WIDTH = 1000;

/** Pan/zoom plus vertical scroll of the lanes, flushed to React together. */
interface View {
  viewport: TimelineViewport;
  scrollY: number;
}

interface Hover {
  event: EventSummary;
  x: number;
  y: number;
}

/** Route component for `/world/timeline`. */
export function TimelinePage() {
  const calendar = useDefaultCalendar();
  if (calendar.isPending) {
    return <p className="p-6 text-sm text-muted-foreground">Loading…</p>;
  }
  if (calendar.error || !calendar.data) {
    return (
      <p className="p-6 text-sm text-destructive">
        {calendar.error?.message ?? "No calendar available."}
      </p>
    );
  }
  // Events carry a calendarId, but worlds only have the default calendar for now.
  return <TimelineView def={calendar.data.definition} />;
}

function importanceRadius(importance: number): number {
  return 3 + clamp(importance, 1, 5);
}

function TimelineView({ def }: { def: CalendarDefinition }) {
  const openEntity = useOpenEntity();
  const createEvent = useCreateEvent();
  const fetchAll = useFetchAllEvents();
  const maintenance = useIndexMaintenance();
  const saveViewport = useTimelineStore((s) => s.saveViewport);
  const pendingFocus = useTimelineStore((s) => s.pendingFocus);
  const setPendingFocus = useTimelineStore((s) => s.setPendingFocus);

  const containerRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const width = size.width || FALLBACK_WIDTH;

  // The live view lives in a ref and is flushed to state once per animation
  // frame, so a burst of wheel/pointer events costs one render, not many.
  const [view, setView] = useState<View>(() => ({
    viewport: useTimelineStore.getState().viewport,
    scrollY: 0,
  }));
  const viewRef = useRef<View>(view);
  const frameRef = useRef(0);
  const widthRef = useRef(width);
  const heightRef = useRef(0);
  const contentHeightRef = useRef(0);

  const [hover, setHover] = useState<Hover | null>(null);
  const [createAt, setCreateAt] = useState<number | null>(null);
  const [fitting, setFitting] = useState(false);

  const update = useCallback((next: View) => {
    viewRef.current = next;
    if (frameRef.current) return;
    frameRef.current = requestAnimationFrame(() => {
      frameRef.current = 0;
      setView(viewRef.current);
    });
  }, []);

  const setViewport = useCallback(
    (viewport: TimelineViewport) => update({ ...viewRef.current, viewport }),
    [update],
  );

  // Track the container size.
  useEffect(() => {
    const el = containerRef.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => {
      if (!entry) return;
      const { width: w, height: h } = entry.contentRect;
      widthRef.current = w || FALLBACK_WIDTH;
      heightRef.current = h;
      setSize({ width: w, height: h });
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  // Remember the place when leaving; cancel any pending frame.
  useEffect(() => {
    return () => {
      cancelAnimationFrame(frameRef.current);
      saveViewport(viewRef.current.viewport);
    };
  }, [saveViewport]);

  // "Show on timeline": frame the requested event once the size is known.
  useEffect(() => {
    if (!pendingFocus || size.width === 0) return;
    setViewport(focusViewport(pendingFocus.startDay, pendingFocus.endDay, size.width));
    setPendingFocus(null);
  }, [pendingFocus, size.width, setViewport, setPendingFocus]);

  // Wheel zoom needs a non-passive listener so the page does not scroll.
  useEffect(() => {
    const svg = svgRef.current;
    if (!svg) return;
    function onWheel(e: WheelEvent) {
      e.preventDefault();
      if (!svg) return;
      const pointerX = e.clientX - svg.getBoundingClientRect().left;
      setViewport(zoomAround(viewRef.current.viewport, widthRef.current, pointerX, e.deltaY));
    }
    svg.addEventListener("wheel", onWheel, { passive: false });
    return () => svg.removeEventListener("wheel", onWheel);
  }, [setViewport]);

  const { viewport, scrollY } = view;
  const range = visibleRange(viewport, width);

  // Fetch only the visible window, padded and snapped so panning reuses the same query.
  const win = queryWindow(range.from, range.to);
  const events = useEvents(win.from, win.to);

  const layout = useMemo(
    () => layoutEvents(events.data ?? [], viewport, width),
    [events.data, viewport, width],
  );
  const ticks = useMemo(
    () => generateTicks(def, range.from - 2, range.to + 2, viewport.daysPerPixel),
    [def, range.from, range.to, viewport.daysPerPixel],
  );

  const contentHeight = layout.laneCount * LANE_HEIGHT;
  const visibleLaneHeight = Math.max(0, size.height - LANES_TOP);
  const maxScroll = Math.max(0, contentHeight - visibleLaneHeight + LANE_HEIGHT / 2);
  const y = Math.min(scrollY, maxScroll);
  useEffect(() => {
    contentHeightRef.current = maxScroll;
  }, [maxScroll]);

  // --- pointer interaction: drag to pan --------------------------------------
  const dragRef = useRef<{
    startX: number;
    startY: number;
    lastX: number;
    lastY: number;
    moved: boolean;
  } | null>(null);

  function onPointerDown(e: React.PointerEvent<SVGSVGElement>) {
    if (e.button !== 0) return;
    dragRef.current = {
      startX: e.clientX,
      startY: e.clientY,
      lastX: e.clientX,
      lastY: e.clientY,
      moved: false,
    };
  }

  function onPointerMove(e: React.PointerEvent<SVGSVGElement>) {
    const drag = dragRef.current;
    if (!drag) return;
    if (!drag.moved) {
      if (Math.hypot(e.clientX - drag.startX, e.clientY - drag.startY) < DRAG_THRESHOLD) return;
      drag.moved = true;
      // Capture only once it is a real drag, so plain clicks still reach the events.
      e.currentTarget.setPointerCapture(e.pointerId);
      setHover(null);
    }
    const dx = e.clientX - drag.lastX;
    const dy = e.clientY - drag.lastY;
    drag.lastX = e.clientX;
    drag.lastY = e.clientY;
    const current = viewRef.current;
    update({
      viewport: panBy(current.viewport, dx),
      scrollY: clamp(current.scrollY - dy, 0, contentHeightRef.current),
    });
  }

  function endDrag(e: React.PointerEvent<SVGSVGElement>) {
    if (dragRef.current?.moved && e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
    dragRef.current = null;
  }

  function onDoubleClick(e: React.PointerEvent<SVGSVGElement> | React.MouseEvent<SVGSVGElement>) {
    if ((e.target as Element).closest("[data-event]")) return;
    const x = e.clientX - e.currentTarget.getBoundingClientRect().left;
    setCreateAt(Math.floor(xToDay(viewRef.current.viewport, widthRef.current, x)));
  }

  function showHover(e: React.PointerEvent, event: EventSummary) {
    if (dragRef.current?.moved) return;
    const box = containerRef.current?.getBoundingClientRect();
    if (!box) return;
    setHover({ event, x: e.clientX - box.left, y: e.clientY - box.top });
  }

  // --- toolbar / keyboard actions --------------------------------------------
  function zoomBy(deltaY: number) {
    setViewport(
      zoomAround(viewRef.current.viewport, widthRef.current, widthRef.current / 2, deltaY),
    );
  }

  async function handleFit() {
    setFitting(true);
    try {
      const all = await fetchAll();
      if (all.length === 0) return;
      const start = Math.min(...all.map((e) => e.startDay));
      const end = Math.max(...all.map((e) => e.endDay ?? e.startDay));
      setViewport(fitRange(start, end, widthRef.current));
    } finally {
      setFitting(false);
    }
  }

  function onKeyDown(e: React.KeyboardEvent) {
    if ((e.target as Element).closest("[data-event], input, button")) return;
    const w = widthRef.current;
    if (e.key === "ArrowLeft") setViewport(panBy(viewRef.current.viewport, w * 0.1));
    else if (e.key === "ArrowRight") setViewport(panBy(viewRef.current.viewport, -w * 0.1));
    else if (e.key === "+" || e.key === "=") zoomBy(-1);
    else if (e.key === "-") zoomBy(1);
    else return;
    e.preventDefault();
  }

  async function handleCreate(name: string) {
    if (createAt === null) return;
    try {
      await createEvent.mutateAsync({ name, startDay: createAt });
      setCreateAt(null);
    } catch {
      // The dialog shows `createEvent.error`; keep it open so the name isn't lost.
    }
  }

  const centerLabel = formatDate(def, Math.floor(viewport.centerDay), "monthYear");

  return (
    <div className="flex min-w-0 flex-1 flex-col" onKeyDown={onKeyDown}>
      <div className="flex h-11 shrink-0 items-center gap-2 border-b px-3">
        <h1 className="text-sm font-semibold">Timeline</h1>
        <span className="text-sm text-muted-foreground" aria-label="Centre of the view">
          {centerLabel}
        </span>
        <div className="ml-auto flex items-center gap-1">
          {maintenance.status && (
            <span
              role="status"
              className={
                maintenance.status.kind === "error"
                  ? "text-xs text-destructive"
                  : "text-xs text-muted-foreground"
              }
            >
              {maintenance.status.message}
            </span>
          )}
          {import.meta.env.DEV && (
            <Button
              variant="ghost"
              size="sm"
              disabled={maintenance.busy}
              onClick={() => void maintenance.seedEvents()}
            >
              <FlaskConical /> Seed 200 events
            </Button>
          )}
          <Button variant="ghost" size="icon-sm" aria-label="Zoom out" onClick={() => zoomBy(1)}>
            <Minus />
          </Button>
          <Button variant="ghost" size="icon-sm" aria-label="Zoom in" onClick={() => zoomBy(-1)}>
            <Plus />
          </Button>
          <Button variant="ghost" size="sm" disabled={fitting} onClick={() => void handleFit()}>
            <Maximize2 /> Fit all events
          </Button>
          <Button variant="ghost" size="sm" onClick={() => setViewport(DEFAULT_VIEWPORT)}>
            <RotateCcw /> Reset view
          </Button>
        </div>
      </div>

      <div ref={containerRef} className="relative min-h-0 flex-1 overflow-hidden select-none">
        <svg
          ref={svgRef}
          role="application"
          aria-label="Timeline. Drag to pan, scroll to zoom, double-click to add an event."
          tabIndex={0}
          className="absolute inset-0 size-full cursor-grab outline-none active:cursor-grabbing"
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={endDrag}
          onPointerCancel={endDrag}
          onDoubleClick={onDoubleClick}
        >
          {/* Transparent backdrop so empty space receives pointer events. */}
          <rect width="100%" height="100%" fill="transparent" />

          {/* Grid lines */}
          {ticks
            .filter((t) => t.major)
            .map((t) => {
              const x = dayToX(viewport, width, t.day);
              return (
                <line
                  key={`g${t.day}`}
                  x1={x}
                  x2={x}
                  y1={AXIS_HEIGHT}
                  y2="100%"
                  className="stroke-border"
                  strokeDasharray="2 4"
                />
              );
            })}

          {/* Events, scrolled vertically */}
          <g transform={`translate(0 ${LANES_TOP - y})`}>
            {layout.items.map((item) => {
              const { event, extent } = item;
              if (extent.end < -20 || extent.start > width + 20) return null;
              const cy = item.lane * LANE_HEIGHT + LANE_HEIGHT / 2;
              const screenY = LANES_TOP - y + cy;
              if (screenY < AXIS_HEIGHT - LANE_HEIGHT || screenY > size.height + LANE_HEIGHT) {
                return null;
              }
              const r = importanceRadius(event.importance);
              const isRange = item.endX !== null;
              const major = event.importance >= 4;

              let marker: React.ReactNode;
              let labelX: number;
              let labelClass = "fill-foreground";
              if (isRange && item.endX !== null) {
                const barWidth = Math.max(item.endX - item.startX, 4);
                marker = (
                  <rect
                    x={item.startX}
                    y={cy - 9}
                    width={barWidth}
                    height={18}
                    rx={4}
                    className="fill-primary stroke-primary"
                    fillOpacity={0.12 + 0.06 * event.importance}
                    strokeWidth={major ? 2 : 1}
                  />
                );
                if (extent.labelInside) {
                  // Keep the label visible while the bar runs off the left edge.
                  labelX = Math.min(
                    Math.max(item.startX + 6, 6),
                    item.startX + barWidth - extent.labelWidth - 6,
                  );
                } else {
                  labelX = item.startX + barWidth + 6;
                }
              } else {
                const cx = item.startX;
                marker = major ? (
                  <polygon
                    points={`${cx},${cy - r} ${cx + r},${cy} ${cx},${cy + r} ${cx - r},${cy}`}
                    className="fill-primary stroke-background"
                  />
                ) : (
                  <circle
                    cx={cx}
                    cy={cy}
                    r={r}
                    className={
                      event.importance >= 3
                        ? "fill-primary stroke-background"
                        : "fill-muted-foreground stroke-background"
                    }
                  />
                );
                labelX = cx + r + 6;
                labelClass = major ? "fill-foreground font-medium" : "fill-foreground";
              }

              return (
                <g
                  key={event.entityId}
                  data-event
                  role="button"
                  tabIndex={0}
                  aria-label={event.name}
                  className="cursor-pointer outline-none focus-visible:[&>*:first-child]:stroke-ring"
                  onClick={() => openEntity(event.entityId)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") openEntity(event.entityId);
                  }}
                  onPointerEnter={(e) => showHover(e, event)}
                  onPointerLeave={() => setHover(null)}
                >
                  {marker}
                  <text x={labelX} y={cy + 4} fontSize={12} className={labelClass}>
                    {event.name}
                  </text>
                </g>
              );
            })}
          </g>

          {/* Axis band, drawn last so events slide beneath it */}
          <rect width="100%" height={AXIS_HEIGHT} className="fill-background" />
          <line x1={0} x2="100%" y1={AXIS_HEIGHT} y2={AXIS_HEIGHT} className="stroke-border" />
          {ticks.map((t) => {
            const x = dayToX(viewport, width, t.day);
            return (
              <g key={`t${t.day}-${t.major}`}>
                <line
                  x1={x}
                  x2={x}
                  y1={t.major ? AXIS_HEIGHT - 12 : AXIS_HEIGHT - 6}
                  y2={AXIS_HEIGHT}
                  className="stroke-muted-foreground"
                />
                {t.label && (
                  <text
                    x={x + 4}
                    y={AXIS_HEIGHT - 16}
                    fontSize={12}
                    className="fill-muted-foreground"
                  >
                    {t.label}
                  </text>
                )}
              </g>
            );
          })}
        </svg>

        {events.data?.length === 0 && (
          <p className="pointer-events-none absolute inset-x-0 top-24 text-center text-sm text-muted-foreground">
            No events in view. Double-click the timeline to add one.
          </p>
        )}

        {hover && (
          <div
            role="tooltip"
            className="pointer-events-none absolute z-10 max-w-xs rounded-md border bg-popover px-3 py-2 text-sm text-popover-foreground shadow-md"
            style={{
              left: Math.min(hover.x + 12, Math.max(0, width - 260)),
              top: hover.y + 16,
            }}
          >
            <div className="font-medium">{hover.event.name}</div>
            <div className="text-xs text-muted-foreground">
              {formatDate(def, hover.event.startDay, "full")}
              {hover.event.endDay !== null && ` – ${formatDate(def, hover.event.endDay, "full")}`}
            </div>
          </div>
        )}
      </div>

      <CreateEventDialog
        open={createAt !== null}
        dateLabel={createAt === null ? "" : formatDate(def, createAt, "full")}
        busy={createEvent.isPending}
        error={createEvent.error?.message ?? null}
        onCancel={() => {
          createEvent.reset();
          setCreateAt(null);
        }}
        onCreate={(name) => void handleCreate(name)}
      />
    </div>
  );
}
