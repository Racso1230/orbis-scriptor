import { describe, expect, it } from "vitest";
import type { EventSummary } from "@/lib/bindings";
import { layoutEvents } from "./layout";

const ev = (id: string, startDay: number, endDay: number | null = null): EventSummary => ({
  entityId: id,
  name: id,
  startDay,
  endDay,
  importance: 1,
  calendarId: "cal",
});
const vp = { centerDay: 0, daysPerPixel: 1 };

describe("layoutEvents", () => {
  it("returns no lanes for no events", () => {
    expect(layoutEvents([], vp, 1000)).toEqual({ items: [], laneCount: 0 });
  });

  it("places points at the middle of their day and bars across whole days", () => {
    const { items } = layoutEvents([ev("p", 10), ev("r", 100, 199)], vp, 1000);
    expect(items[0]?.startX).toBe(500 + 10.5);
    expect(items[0]?.endX).toBeNull();
    expect(items[1]?.startX).toBe(600);
    expect(items[1]?.endX).toBe(700);
  });

  it("stacks a point that falls inside a war into the next lane", () => {
    const { items, laneCount } = layoutEvents(
      [ev("war", 0, 400), ev("coronation", 100), ev("peace", 900)],
      vp,
      1000,
    );
    const lane = Object.fromEntries(items.map((i) => [i.event.entityId, i.lane]));
    expect(lane).toEqual({ war: 0, coronation: 1, peace: 0 });
    expect(laneCount).toBe(2);
  });
});
