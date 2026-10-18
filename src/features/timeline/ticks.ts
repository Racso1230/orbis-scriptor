import type { CalendarDefinition } from "@/lib/bindings";
import {
  daysBeforeYear,
  daysPerYear,
  euclidMod,
  formatDate,
  startOfMonth,
  startOfYear,
  toDate,
  yearIndexOfDay,
} from "./calendar";

export interface Tick {
  day: number;
  /** Empty for minor ticks. */
  label: string;
  major: boolean;
}

type UnitKind = "day" | "month" | "year";

/** One candidate tick spacing, e.g. "every 5 days" or "every 10 years". */
export interface TickUnit {
  kind: UnitKind;
  step: number;
  /** Approximate length in days (months and leap years vary, so this is an average). */
  span: number;
}

/** Major ticks are never closer than this many pixels. */
export const MIN_MAJOR_SPACING = 80;
/** Minor ticks are only drawn when they are at least this far apart. */
export const MIN_MINOR_SPACING = 12;
/** Hard cap so a pathological calendar cannot freeze the UI. */
const MAX_TICKS = 600;

const DAY_STEPS = [1, 2, 5];
const MONTH_STEPS = [1, 2, 3, 6];
// 1, 2, 5, 10, 20, 50, ... 5,000,000 years: covers decade / century / millennium scales.
const YEAR_STEPS = Array.from({ length: 7 }, (_, e) => [1, 2, 5].map((m) => m * 10 ** e)).flat();

/** Mean year length in days, leap days included (measured over a long stretch of years). */
function averageYearLength(def: CalendarDefinition): number {
  const base = daysPerYear(def);
  if (!def.leap || base <= 0) return base;
  const sample = 10000;
  return daysBeforeYear(def, sample) / sample;
}

/** All candidate units for a calendar, shortest first. */
export function tickUnits(def: CalendarDefinition): TickUnit[] {
  const year = averageYearLength(def);
  const avgMonth = year / Math.max(1, def.months.length);
  const units: TickUnit[] = [
    ...DAY_STEPS.map((step) => ({ kind: "day" as const, step, span: step })),
    ...MONTH_STEPS.filter((step) => step <= def.months.length).map((step) => ({
      kind: "month" as const,
      step,
      span: avgMonth * step,
    })),
    ...YEAR_STEPS.map((step) => ({ kind: "year" as const, step, span: year * step })),
  ];
  return units.sort((a, b) => a.span - b.span);
}

/** Index of the first unit whose spacing is at least `minSpacing` pixels (else the last). */
export function chooseUnitIndex(
  units: readonly TickUnit[],
  daysPerPixel: number,
  minSpacing = MIN_MAJOR_SPACING,
): number {
  const i = units.findIndex((u) => u.span / daysPerPixel >= minSpacing);
  return i === -1 ? units.length - 1 : i;
}

/**
 * Days (not yet labelled) at which `unit` has a boundary within `[from, to]`.
 * Years and months are real calendar boundaries, so leap days and uneven year
 * lengths are respected; the range is padded by a year on each side.
 */
function unitDays(def: CalendarDefinition, unit: TickUnit, from: number, to: number): number[] {
  const days: number[] = [];

  if (unit.kind === "day") {
    for (let d = Math.floor(from) - 1; d <= Math.ceil(to) + 1; d++) {
      if (euclidMod(toDate(def, d).day - 1, unit.step) === 0) days.push(d);
      if (days.length > MAX_TICKS) break;
    }
    return days;
  }

  const firstYear = yearIndexOfDay(def, Math.floor(from)) - 1;
  const lastYear = yearIndexOfDay(def, Math.ceil(to)) + 1;
  for (let a = firstYear; a <= lastYear; a++) {
    const yearStart = startOfYear(def, a);
    if (unit.kind === "year") {
      // Align to the displayed year number (so a 10-year step hits 10, 20, 30...).
      const shown = toDate(def, yearStart).year;
      if (unit.step === 1 || euclidMod(shown, unit.step) === 0) days.push(yearStart);
    } else {
      for (let m = 0; m < def.months.length; m += unit.step) {
        days.push(startOfMonth(def, a, m));
      }
    }
    if (days.length > MAX_TICKS) break;
  }
  return days;
}

function labelFor(def: CalendarDefinition, unit: TickUnit, day: number): string {
  switch (unit.kind) {
    case "year":
      return formatDate(def, day, "year");
    case "month":
      return formatDate(def, day, toDate(def, day).month === 1 ? "monthYear" : "month");
    case "day":
      return formatDate(def, day, toDate(def, day).day === 1 ? "month" : "day");
  }
}

/**
 * Ticks for the visible day range `[from, to]` at `daysPerPixel`. Major ticks
 * are labelled and about 80-200px apart, aligned to calendar boundaries; minor
 * ticks (unlabelled) come from the next finer unit when there is room.
 */
export function generateTicks(
  def: CalendarDefinition,
  from: number,
  to: number,
  daysPerPixel: number,
): Tick[] {
  const units = tickUnits(def);
  const majorIndex = chooseUnitIndex(units, daysPerPixel);
  const majorUnit = units[majorIndex];
  if (!majorUnit) return [];

  const major = unitDays(def, majorUnit, from, to).map((day): Tick => ({
    day,
    label: labelFor(def, majorUnit, day),
    major: true,
  }));
  const majorDays = new Set(major.map((t) => t.day));

  // Finest unit still in the "comfortable" range that is shorter than the major one.
  let minorUnit: TickUnit | undefined;
  for (let i = majorIndex - 1; i >= 0; i--) {
    const u = units[i];
    if (u && u.span < majorUnit.span && u.span / daysPerPixel >= MIN_MINOR_SPACING) {
      minorUnit = u;
      break;
    }
  }
  const minor = minorUnit
    ? unitDays(def, minorUnit, from, to)
        .filter((day) => !majorDays.has(day))
        .map((day): Tick => ({ day, label: "", major: false }))
    : [];

  return [...major, ...minor].sort((a, b) => a.day - b.day);
}
