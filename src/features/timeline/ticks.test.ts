import { describe, expect, it } from "vitest";
import type { CalendarDefinition } from "@/lib/bindings";
import { fromDate } from "./calendar";
import { generateTicks, tickUnits } from "./ticks";

const def: CalendarDefinition = {
  months: Array.from({ length: 12 }, (_, i) => ({ name: `Month ${i + 1}`, days: 30 })),
  weekdays: [],
  eras: [{ name: "After Founding", abbreviation: "AF", startDay: 0 }],
};

const majors = (from: number, to: number, dpp: number) =>
  generateTicks(def, from, to, dpp).filter((t) => t.major && t.day >= from && t.day <= to);

describe("generateTicks", () => {
  it("uses yearly ticks at a few years across", () => {
    // Year 1 starts at day 0, so year 0 starts at day -360.
    const ticks = majors(-360, 1800, 3);
    expect(ticks.map((t) => t.label).slice(0, 3)).toEqual(["0 AF", "1 AF", "2 AF"]);
    // Year boundaries are multiples of 360 days.
    expect(ticks.every((t) => t.day % 360 === 0)).toBe(true);
  });

  it("aligns decade ticks to round year numbers", () => {
    const ticks = majors(0, 360 * 120, 36); // 10px per year
    const labels = ticks.map((t) => t.label);
    expect(labels).toContain("50 AF");
    expect(labels).toContain("100 AF");
    // Every label is a multiple of the same step.
    expect(ticks.every((t) => Number.parseInt(t.label, 10) % 10 === 0)).toBe(true);
  });

  it("uses century-sized steps when zoomed out", () => {
    const ticks = majors(0, 360 * 1000, 200);
    expect(ticks.map((t) => t.label)).toContain("500 AF");
    const gaps = ticks.slice(1).map((t, i) => t.day - (ticks[i]?.day ?? 0));
    expect(Math.min(...gaps)).toBeGreaterThanOrEqual(360 * 50);
  });

  it("uses month ticks at month scale, with the year on month 1", () => {
    const start = fromDate(def, 0, 5, 1, 1);
    const ticks = majors(start, start + 100, 0.3);
    expect(ticks[0]?.label).toBe("Month 1, 5 AF");
    expect(ticks[1]?.label).toBe("Month 2");
  });

  it("shows day numbers at day scale, with the month name on day 1", () => {
    const start = fromDate(def, 0, 5, 3, 28);
    const ticks = majors(start, start + 6, 0.01);
    expect(ticks.map((t) => t.label)).toEqual(["28", "29", "30", "Month 4", "2", "3", "4"]);
  });

  it("keeps major ticks 80 to 250 pixels apart at many zoom levels", () => {
    for (const dpp of [0.004, 0.02, 0.1, 0.5, 2, 8, 30, 120, 200]) {
      const ticks = majors(-5000, 5000 + dpp * 1500, dpp);
      for (let i = 1; i < ticks.length; i++) {
        const px = ((ticks[i]?.day ?? 0) - (ticks[i - 1]?.day ?? 0)) / dpp;
        // Months and day-1 boundaries are uneven only at the edges; allow slack.
        expect(px).toBeGreaterThanOrEqual(50);
        expect(px).toBeLessThanOrEqual(400);
      }
    }
  });

  it("adds unlabelled minor ticks between majors when there is room", () => {
    const ticks = generateTicks(def, 0, 1800, 3);
    const minor = ticks.filter((t) => !t.major);
    expect(minor.length).toBeGreaterThan(0);
    expect(minor.every((t) => t.label === "")).toBe(true);
    // Sorted and no duplicate days.
    const days = ticks.map((t) => t.day);
    expect(days).toEqual([...new Set(days)].sort((a, b) => a - b));
  });

  it("handles negative days", () => {
    const ticks = majors(-1000, 0, 3);
    expect(ticks.some((t) => t.label === "0 AF")).toBe(true);
  });
});

describe("tickUnits", () => {
  it("is sorted from days up to millennia", () => {
    const units = tickUnits(def);
    expect(units[0]).toMatchObject({ kind: "day", step: 1 });
    expect(units.map((u) => u.span)).toEqual([...units.map((u) => u.span)].sort((a, b) => a - b));
    expect(units.some((u) => u.kind === "year" && u.step === 1000)).toBe(true);
  });
});
