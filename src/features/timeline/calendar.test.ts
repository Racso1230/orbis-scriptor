import { describe, expect, it } from "vitest";
import type { CalendarDefinition } from "@/lib/bindings";
import {
  daysBeforeYear,
  daysInMonth,
  daysInYear,
  daysPerYear,
  formatDate,
  fromDate,
  isLeapYear,
  startOfMonth,
  startOfYear,
  toDate,
  yearIndexOf,
} from "./calendar";

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

// Gregorian-style: February (index 1) gains a day; year 1 is index 0, so year 4 is index 3.
const gregorian: CalendarDefinition = {
  months: [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31].map((days, i) => ({
    name: `M${i + 1}`,
    days,
  })),
  weekdays: ["A", "B", "C"],
  eras: [{ name: "Common", abbreviation: "CE", startDay: 0 }],
  epochOffset: 0,
  leap: {
    monthIndex: 1,
    extraDays: 1,
    rules: [
      { every: 4, add: true },
      { every: 100, add: false },
      { every: 400, add: true },
    ],
  },
};

describe("leap years", () => {
  it("follows the 4/100/400 cycle on the displayed year (index + 1)", () => {
    const leap = (year: number) => isLeapYear(gregorian, year - 1);
    expect([leap(4), leap(100), leap(400), leap(2024), leap(1900), leap(2000)]).toEqual([
      true,
      false,
      true,
      true,
      false,
      true,
    ]);
    expect(leap(0)).toBe(true); // year 0 is divisible by every cycle, the largest adds
    expect(leap(-4)).toBe(true);
    expect(leap(3)).toBe(false);
  });

  it("lengthens February and the year in leap years", () => {
    expect(daysInMonth(gregorian, 3, 2)).toBe(29);
    expect(daysInMonth(gregorian, 2, 2)).toBe(28);
    expect(daysInYear(gregorian, 3)).toBe(366);
    expect(daysInYear(gregorian, 99)).toBe(365);
    expect(daysBeforeYear(gregorian, 4)).toBe(4 * 365 + 1);
  });

  it("round trips the leap day and the days around it", () => {
    const feb29 = fromDate(gregorian, 0, 4, 2, 29);
    expect(toDate(gregorian, feb29)).toMatchObject({ month: 2, day: 29, year: 4 });
    expect(toDate(gregorian, feb29 + 1)).toMatchObject({ month: 3, day: 1, year: 4 });
    expect(fromDate(gregorian, 0, 4, 3, 1)).toBe(feb29 + 1);
  });

  it("finds year starts consistently, also for negative days", () => {
    for (const a of [-401, -5, -1, 0, 1, 3, 99, 399]) {
      const start = startOfYear(gregorian, a);
      expect(yearIndexOf(gregorian, start)).toBe(a);
      expect(yearIndexOf(gregorian, start - 1)).toBe(a - 1);
      expect(startOfMonth(gregorian, a, 0)).toBe(start);
      expect(startOfMonth(gregorian, a, 2)).toBe(start + 59 + (isLeapYear(gregorian, a) ? 1 : 0));
    }
  });
});
