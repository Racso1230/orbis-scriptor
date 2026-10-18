import type { CalendarDefinition, LeapCycle, ValidationIssue } from "@/lib/bindings";

// The calendar editor edits a "draft": the same data as a CalendarDefinition,
// but numbers are kept as the text the user typed (so half-typed input like ""
// or "-" survives) and rows carry a stable `key` (so reordering keeps React's
// inputs attached to the right row). Everything here is pure and unit-tested.

let keyCounter = 0;
/** A fresh row key. Only used for React keys, never sent to the backend. */
export function newKey(): string {
  keyCounter += 1;
  return `row-${keyCounter}`;
}

export interface MonthRow {
  key: string;
  name: string;
  days: string;
}
export interface WeekdayRow {
  key: string;
  name: string;
}
export interface EraRow {
  key: string;
  name: string;
  abbreviation: string;
  /** Absolute day number on which the era starts. */
  startDay: string;
}
export interface LeapRuleRow {
  key: string;
  every: string;
  add: boolean;
}
export interface LeapDraft {
  enabled: boolean;
  /** 0-based index of the month that gains the extra days. */
  monthIndex: number;
  extraDays: string;
  rules: LeapRuleRow[];
}
export interface CalendarDraft {
  name: string;
  months: MonthRow[];
  weekdays: WeekdayRow[];
  eras: EraRow[];
  epochOffset: string;
  leap: LeapDraft;
}

/** A starting point for "New calendar": 12 months of 30 days and one era. */
export function newDraft(): CalendarDraft {
  return draftFromCalendar("New calendar", {
    months: Array.from({ length: 12 }, (_, i) => ({ name: `Month ${i + 1}`, days: 30 })),
    weekdays: [],
    eras: [{ name: "First Age", abbreviation: "FA", startDay: 0 }],
    epochOffset: 0,
    leap: null,
  });
}

export function draftFromCalendar(name: string, def: CalendarDefinition): CalendarDraft {
  const leap = def.leap;
  return {
    name,
    months: def.months.map((m) => ({ key: newKey(), name: m.name, days: String(m.days) })),
    weekdays: def.weekdays.map((w) => ({ key: newKey(), name: w })),
    eras: def.eras.map((e) => ({
      key: newKey(),
      name: e.name,
      abbreviation: e.abbreviation ?? "",
      startDay: String(e.startDay),
    })),
    epochOffset: String(def.epochOffset ?? 0),
    leap: {
      enabled: leap != null,
      monthIndex: leap?.monthIndex ?? 0,
      extraDays: String(leap?.extraDays ?? 1),
      rules: (leap?.rules ?? []).map((r) => ({
        key: newKey(),
        every: String(r.every),
        add: r.add,
      })),
    },
  };
}

/** A whole number from typed text, or null when it is empty or not a whole number. */
export function parseWhole(text: string): number | null {
  const trimmed = text.trim();
  if (trimmed === "" || !/^[+-]?\d+$/.test(trimmed)) return null;
  const n = Number(trimmed);
  return Number.isSafeInteger(n) ? n : null;
}

const NOT_A_NUMBER = "Enter a whole number.";

export interface ParsedDraft {
  /** The definition to send to the backend; null while any number is unreadable. */
  definition: CalendarDefinition | null;
  /** Problems found locally (unreadable numbers, empty name), with backend-style field paths. */
  issues: ValidationIssue[];
}

/**
 * Draft -> definition. Only checks that typed numbers can be read; the real
 * rules (lengths, ordering, leap cycles...) are the backend's, via the preview.
 */
export function parseDraft(draft: CalendarDraft): ParsedDraft {
  const issues: ValidationIssue[] = [];
  const bad = (field: string, message = NOT_A_NUMBER) => issues.push({ field, message });

  if (draft.name.trim() === "") bad("name", "Give the calendar a name.");

  const months = draft.months.map((m, i) => {
    const days = parseWhole(m.days);
    if (days === null) bad(`months[${i}].days`);
    return { name: m.name.trim(), days: days ?? 0 };
  });
  const eras = draft.eras.map((e, i) => {
    const startDay = parseWhole(e.startDay);
    if (startDay === null) bad(`eras[${i}].startDay`);
    const abbreviation = e.abbreviation.trim();
    return {
      name: e.name.trim(),
      abbreviation: abbreviation === "" ? null : abbreviation,
      startDay: startDay ?? 0,
    };
  });
  const epochOffset = parseWhole(draft.epochOffset);
  if (epochOffset === null) bad("epochOffset");

  let leap: CalendarDefinition["leap"] = null;
  if (draft.leap.enabled) {
    const extraDays = parseWhole(draft.leap.extraDays);
    if (extraDays === null) bad("leap.extraDays");
    const rules: LeapCycle[] = draft.leap.rules.map((r, i) => {
      const every = parseWhole(r.every);
      if (every === null) bad(`leap.rules[${i}].every`);
      return { every: every ?? 0, add: r.add };
    });
    leap = { monthIndex: draft.leap.monthIndex, extraDays: extraDays ?? 0, rules };
  }

  const definition: CalendarDefinition | null =
    issues.length === 0
      ? {
          months,
          weekdays: draft.weekdays.map((w) => w.name.trim()),
          eras,
          epochOffset: epochOffset ?? 0,
          leap,
        }
      : null;
  return { definition, issues };
}

/** Stable text for "has anything changed?" (row keys are left out). */
export function draftSignature(draft: CalendarDraft): string {
  return JSON.stringify({
    name: draft.name,
    months: draft.months.map(({ name, days }) => [name, days]),
    weekdays: draft.weekdays.map((w) => w.name),
    eras: draft.eras.map(({ name, abbreviation, startDay }) => [name, abbreviation, startDay]),
    epochOffset: draft.epochOffset,
    leap: {
      ...draft.leap,
      rules: draft.leap.rules.map(({ every, add }) => [every, add]),
    },
  });
}

// --- validation issues ------------------------------------------------------

/** First message per field path, for showing next to the inputs. */
export function issuesByField(issues: readonly ValidationIssue[]): Record<string, string> {
  const map: Record<string, string> = {};
  for (const issue of issues) {
    if (!(issue.field in map)) map[issue.field] = issue.message;
  }
  return map;
}

/**
 * Turns the backend's `"field: message; field: message"` validation error back
 * into issues. Text that does not look like that becomes one issue with an
 * empty field (shown as a general message).
 */
export function parseSaveError(message: string): ValidationIssue[] {
  return message
    .split(";")
    .map((part) => part.trim())
    .filter((part) => part !== "")
    .map((part) => {
      const match = /^([A-Za-z][\w.[\]]*): (.*)$/.exec(part);
      return match
        ? { field: match[1] ?? "", message: match[2] ?? "" }
        : { field: "", message: part };
    });
}

// --- leap presets -----------------------------------------------------------

export interface LeapPreset {
  id: "none" | "every4" | "gregorian";
  label: string;
  rules: LeapCycle[];
}

export const LEAP_PRESETS: readonly LeapPreset[] = [
  { id: "none", label: "None", rules: [] },
  { id: "every4", label: "Every 4 years", rules: [{ every: 4, add: true }] },
  {
    id: "gregorian",
    label: "Gregorian (4/100/400)",
    rules: [
      { every: 4, add: true },
      { every: 100, add: false },
      { every: 400, add: true },
    ],
  },
];

/** Applies a preset: "None" turns leap years off, the others switch them on with those rules. */
export function applyLeapPreset(draft: CalendarDraft, id: LeapPreset["id"]): CalendarDraft {
  const preset = LEAP_PRESETS.find((p) => p.id === id);
  if (!preset || id === "none") return { ...draft, leap: { ...draft.leap, enabled: false } };
  const lastMonth = Math.max(0, draft.months.length - 1);
  return {
    ...draft,
    leap: {
      enabled: true,
      monthIndex: Math.min(draft.leap.monthIndex, lastMonth),
      extraDays: parseWhole(draft.leap.extraDays) ? draft.leap.extraDays : "1",
      rules: preset.rules.map((r) => ({ key: newKey(), every: String(r.every), add: r.add })),
    },
  };
}

/** The preset the draft's leap rule matches exactly, or null for a custom rule. */
export function detectLeapPreset(leap: LeapDraft): LeapPreset["id"] | null {
  if (!leap.enabled) return "none";
  const text = JSON.stringify(leap.rules.map((r) => [r.every.trim(), r.add]));
  const found = LEAP_PRESETS.find(
    (p) => p.id !== "none" && JSON.stringify(p.rules.map((r) => [String(r.every), r.add])) === text,
  );
  return found?.id ?? null;
}

// --- list editing -----------------------------------------------------------

/** Moves the item at `index` by `delta` places (clamped); returns a new array. */
export function moveItem<T>(items: readonly T[], index: number, delta: number): T[] {
  const target = index + delta;
  if (index < 0 || index >= items.length || target < 0 || target >= items.length) {
    return [...items];
  }
  const copy = [...items];
  const [item] = copy.splice(index, 1);
  if (item !== undefined) copy.splice(target, 0, item);
  return copy;
}

/** Reorders months; the leap month stays attached to the month it was on. */
export function moveMonth(draft: CalendarDraft, index: number, delta: number): CalendarDraft {
  const target = index + delta;
  if (target < 0 || target >= draft.months.length) return draft;
  let monthIndex = draft.leap.monthIndex;
  if (monthIndex === index) monthIndex = target;
  else if (monthIndex === target) monthIndex = index;
  return {
    ...draft,
    months: moveItem(draft.months, index, delta),
    leap: { ...draft.leap, monthIndex },
  };
}

export function addMonth(draft: CalendarDraft): CalendarDraft {
  const number = draft.months.length + 1;
  const last = draft.months[draft.months.length - 1];
  return {
    ...draft,
    months: [...draft.months, { key: newKey(), name: `Month ${number}`, days: last?.days ?? "30" }],
  };
}

/** Removes a month; the leap month index follows (or is clamped if its month was removed). */
export function removeMonth(draft: CalendarDraft, index: number): CalendarDraft {
  const months = draft.months.filter((_, i) => i !== index);
  let monthIndex = draft.leap.monthIndex;
  if (index < monthIndex) monthIndex -= 1;
  monthIndex = Math.max(0, Math.min(monthIndex, months.length - 1));
  return { ...draft, months, leap: { ...draft.leap, monthIndex } };
}

// --- summaries --------------------------------------------------------------

/** Year lengths for the preview: normal, and leap (null when no year is ever leap). */
export function yearLengths(def: CalendarDefinition): { normal: number; leap: number | null } {
  const normal = def.months.reduce((sum, m) => sum + m.days, 0);
  const hasLeap = def.leap != null && def.leap.rules.some((r) => r.add);
  return { normal, leap: hasLeap && def.leap ? normal + def.leap.extraDays : null };
}

/** "Name (copy)", then "Name (copy 2)"... the first one not already taken (case-insensitive). */
export function uniqueCopyName(name: string, existing: readonly string[]): string {
  const taken = new Set(existing.map((n) => n.toLowerCase()));
  let candidate = `${name} (copy)`;
  for (let n = 2; taken.has(candidate.toLowerCase()); n++) candidate = `${name} (copy ${n})`;
  return candidate;
}
