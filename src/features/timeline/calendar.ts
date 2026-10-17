import type { CalendarDefinition, DateStyle } from "@/lib/bindings";

// A TypeScript port of the Rust calendar module (to_date / from_date / format),
// so tick labels and date editors need no round trip to the backend.
// Day numbers are plain JS numbers (exact up to 2^53, far beyond any world).

/** A day number broken into calendar parts (like the Rust `CalendarDate`, minus weekday). */
export interface DateParts {
  eraIndex: number;
  eraName: string;
  /** Era label used in text: the abbreviation if there is one, else the name. */
  eraLabel: string;
  /** Year within the era; 1 is the first year, dates before the first era go 0, -1, ... */
  year: number;
  /** 1-based. */
  month: number;
  monthName: string;
  /** 1-based day of the month. */
  day: number;
}

/** Division rounding towards negative infinity (JS `/` plus `Math.floor`). */
export function floorDiv(a: number, b: number): number {
  return Math.floor(a / b);
}

/** Remainder that is never negative (JS `%` keeps the sign of the dividend). */
export function euclidMod(a: number, b: number): number {
  return ((a % b) + b) % b;
}

/** Days in a year: the sum of all month lengths. */
export function daysPerYear(def: CalendarDefinition): number {
  return def.months.reduce((sum, m) => sum + m.days, 0);
}

/** Day-of-year (0-based) on which a month (0-based index) starts. */
export function monthStartOffset(def: CalendarDefinition, monthIndex: number): number {
  let offset = 0;
  for (let i = 0; i < monthIndex; i++) offset += def.months[i]?.days ?? 0;
  return offset;
}

/** Index of the era a day falls in: the last era starting on or before it (else the first). */
export function eraIndexFor(def: CalendarDefinition, day: number): number {
  let index = 0;
  def.eras.forEach((era, i) => {
    if (era.startDay <= day) index = i;
  });
  return index;
}

export function eraLabel(def: CalendarDefinition, eraIndex: number): string {
  const era = def.eras[eraIndex];
  return era ? (era.abbreviation ?? era.name) : "";
}

/** Day number -> calendar parts. */
export function toDate(def: CalendarDefinition, day: number): DateParts {
  const yearLength = daysPerYear(def);
  const absYear = floorDiv(day, yearLength);
  const dayOfYear = euclidMod(day, yearLength);

  const eraIndex = eraIndexFor(def, day);
  const era = def.eras[eraIndex];
  const year = absYear - floorDiv(era?.startDay ?? 0, yearLength) + 1;

  // Walk the months until the day-of-year falls inside one.
  let remaining = dayOfYear;
  let monthIndex = 0;
  for (let i = 0; i < def.months.length; i++) {
    monthIndex = i;
    const length = def.months[i]?.days ?? 0;
    if (remaining < length) break;
    remaining -= length;
  }

  return {
    eraIndex,
    eraName: era?.name ?? "",
    eraLabel: eraLabel(def, eraIndex),
    year,
    month: monthIndex + 1,
    monthName: def.months[monthIndex]?.name ?? "",
    day: remaining + 1,
  };
}

/** Calendar parts -> day number (the inverse of `toDate`). `month` and `day` are 1-based. */
export function fromDate(
  def: CalendarDefinition,
  eraIndex: number,
  year: number,
  month: number,
  day: number,
): number {
  const yearLength = daysPerYear(def);
  const era = def.eras[eraIndex];
  const absYear = year - 1 + floorDiv(era?.startDay ?? 0, yearLength);
  return absYear * yearLength + monthStartOffset(def, month - 1) + (day - 1);
}

/** Formats a day number in one of the backend's `DateStyle`s. */
export function formatDate(def: CalendarDefinition, day: number, style: DateStyle): string {
  return formatParts(toDate(def, day), style);
}

export function formatParts(p: DateParts, style: DateStyle): string {
  switch (style) {
    case "full":
      return `${p.day} ${p.monthName}, ${p.year} ${p.eraLabel}`.trim();
    case "short":
      return `${p.day}/${p.month}/${p.year} ${p.eraLabel}`.trim();
    case "monthYear":
      return `${p.monthName}, ${p.year} ${p.eraLabel}`.trim();
    case "year":
      return `${p.year} ${p.eraLabel}`.trim();
    case "month":
      return p.monthName;
    case "day":
      return String(p.day);
  }
}
