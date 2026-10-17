import { describe, expect, it } from "vitest";
import type { CalendarDefinition } from "@/lib/bindings";
import { daysPerYear, formatDate, fromDate, toDate } from "./calendar";

const months = Array.from({ length: 12 }, (_, i) => ({ name: `Month ${i + 1}`, days: 30 }));
const def: CalendarDefinition = {
  months,
  weekdays: [],
  eras: [{ name: "After Founding", abbreviation: "AF", startDay: 0 }],
};

describe("toDate", () => {
  it("matches the backend examples", () => {
    expect(formatDate(def, 0, "full")).toBe("1 Month 1, 1 AF");
    expect(formatDate(def, 359, "full")).toBe("30 Month 12, 1 AF");
    expect(formatDate(def, 360, "full")).toBe("1 Month 1, 2 AF");
    expect(formatDate(def, -1, "full")).toBe("30 Month 12, 0 AF");
  });

  it("computes the year length", () => {
    expect(daysPerYear(def)).toBe(360);
  });

  it("formats every style", () => {
    const day = fromDate(def, 0, 1203, 3, 15);
    expect(formatDate(def, day, "full")).toBe("15 Month 3, 1203 AF");
    expect(formatDate(def, day, "short")).toBe("15/3/1203 AF");
    expect(formatDate(def, day, "monthYear")).toBe("Month 3, 1203 AF");
    expect(formatDate(def, day, "year")).toBe("1203 AF");
    expect(formatDate(def, day, "month")).toBe("Month 3");
    expect(formatDate(def, day, "day")).toBe("15");
  });

  it("falls back to the era name without an abbreviation", () => {
    const d = { ...def, eras: [{ name: "Age", abbreviation: null, startDay: 0 }] };
    expect(formatDate(d, 0, "year")).toBe("1 Age");
  });

  it("handles uneven months and multiple eras", () => {
    const custom: CalendarDefinition = {
      months: [
        { name: "A", days: 10 },
        { name: "B", days: 20 },
      ],
      weekdays: [],
      eras: [
        { name: "First", abbreviation: "FE", startDay: 0 },
        { name: "Second", abbreviation: "SE", startDay: 300 },
      ],
    };
    expect(toDate(custom, 9)).toMatchObject({ month: 1, day: 10 });
    expect(toDate(custom, 10)).toMatchObject({ month: 2, day: 1 });
    // Day 300 starts the second era, which starts at its year 1.
    expect(toDate(custom, 300)).toMatchObject({ eraName: "Second", year: 1, month: 1, day: 1 });
    expect(toDate(custom, 299)).toMatchObject({ eraName: "First", year: 10 });
  });
});

describe("fromDate round trip", () => {
  it("is the inverse of toDate across negative and positive days", () => {
    for (const day of [-100000, -721, -360, -1, 0, 1, 29, 30, 359, 360, 1234567]) {
      const p = toDate(def, day);
      expect(fromDate(def, p.eraIndex, p.year, p.month, p.day)).toBe(day);
    }
  });
});
