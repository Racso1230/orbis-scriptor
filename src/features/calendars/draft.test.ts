import { describe, expect, it } from "vitest";
import type { CalendarDefinition } from "@/lib/bindings";
import {
  LEAP_PRESETS,
  addMonth,
  applyLeapPreset,
  detectLeapPreset,
  draftFromCalendar,
  draftSignature,
  issuesByField,
  moveItem,
  moveMonth,
  newDraft,
  parseDraft,
  parseSaveError,
  parseWhole,
  removeMonth,
  uniqueCopyName,
  yearLengths,
} from "./draft";
import { ELVEN, STANDARD } from "./test-calendars";

describe("parseDraft", () => {
  it("round trips a definition through a draft unchanged", () => {
    for (const calendar of [STANDARD, ELVEN]) {
      const draft = draftFromCalendar(calendar.name, calendar.definition);
      expect(parseDraft(draft)).toEqual({ definition: calendar.definition, issues: [] });
    }
  });

  it("trims names, turns an empty abbreviation into null and reads typed numbers", () => {
    const draft = draftFromCalendar("Test", STANDARD.definition);
    draft.months[0] = { ...draft.months[0]!, name: "  Frost ", days: " 31 " };
    draft.eras[0] = { ...draft.eras[0]!, abbreviation: "  ", startDay: "-5" };
    draft.epochOffset = "+12";
    const { definition } = parseDraft(draft);
    expect(definition?.months[0]).toEqual({ name: "Frost", days: 31 });
    expect(definition?.eras[0]).toMatchObject({ abbreviation: null, startDay: -5 });
    expect(definition?.epochOffset).toBe(12);
  });

  it("leaves leap out when disabled", () => {
    const draft = draftFromCalendar("Test", ELVEN.definition);
    draft.leap.enabled = false;
    expect(parseDraft(draft).definition?.leap).toBeNull();
  });

  it("reports unreadable numbers with backend-style field paths and no definition", () => {
    const draft = draftFromCalendar("Test", ELVEN.definition);
    draft.months[1] = { ...draft.months[1]!, days: "" };
    draft.eras[1] = { ...draft.eras[1]!, startDay: "1.5" };
    draft.epochOffset = "abc";
    draft.leap.extraDays = "";
    draft.leap.rules[0] = { ...draft.leap.rules[0]!, every: "x" };
    draft.name = "   ";
    const result = parseDraft(draft);
    expect(result.definition).toBeNull();
    expect(result.issues.map((i) => i.field).sort()).toEqual(
      [
        "name",
        "months[1].days",
        "eras[1].startDay",
        "epochOffset",
        "leap.extraDays",
        "leap.rules[0].every",
      ].sort(),
    );
  });

  it("parseWhole only accepts whole numbers", () => {
    expect([parseWhole("12"), parseWhole("-3"), parseWhole(" 7 ")]).toEqual([12, -3, 7]);
    expect([parseWhole(""), parseWhole("1.5"), parseWhole("1e3"), parseWhole("-")]).toEqual([
      null,
      null,
      null,
      null,
    ]);
  });
});

describe("validation issues", () => {
  it("maps issues to their fields, keeping the first message per field", () => {
    const map = issuesByField([
      { field: "months[2].days", message: "must be at least 1" },
      { field: "months[2].days", message: "second" },
      { field: "leap.rules[1].every", message: "must divide the next cycle" },
    ]);
    expect(map).toEqual({
      "months[2].days": "must be at least 1",
      "leap.rules[1].every": "must divide the next cycle",
    });
  });

  it("splits the backend's validation message back into field issues", () => {
    expect(parseSaveError("months: add at least one month; eras[0].name: required")).toEqual([
      { field: "months", message: "add at least one month" },
      { field: "eras[0].name", message: "required" },
    ]);
    expect(parseSaveError("name: A calendar with that name already exists")).toEqual([
      { field: "name", message: "A calendar with that name already exists" },
    ]);
    expect(parseSaveError("Something unexpected went wrong")).toEqual([
      { field: "", message: "Something unexpected went wrong" },
    ]);
  });
});

describe("leap presets", () => {
  it("applies the Gregorian preset and recognises it again", () => {
    const draft = applyLeapPreset(newDraft(), "gregorian");
    expect(draft.leap.enabled).toBe(true);
    expect(parseDraft(draft).definition?.leap).toEqual({
      monthIndex: 0,
      extraDays: 1,
      rules: LEAP_PRESETS.find((p) => p.id === "gregorian")?.rules,
    });
    expect(detectLeapPreset(draft.leap)).toBe("gregorian");
  });

  it("applies every-4-years, and None switches leap years off", () => {
    const every4 = applyLeapPreset(newDraft(), "every4");
    expect(detectLeapPreset(every4.leap)).toBe("every4");
    const none = applyLeapPreset(every4, "none");
    expect(none.leap.enabled).toBe(false);
    expect(detectLeapPreset(none.leap)).toBe("none");
  });

  it("calls an edited rule list custom", () => {
    const draft = applyLeapPreset(newDraft(), "every4");
    draft.leap.rules[0] = { ...draft.leap.rules[0]!, every: "5" };
    expect(detectLeapPreset(draft.leap)).toBeNull();
  });

  it("keeps a valid leap month and extra days when applying a preset", () => {
    const draft = draftFromCalendar("x", ELVEN.definition);
    const next = applyLeapPreset(draft, "gregorian");
    expect(next.leap.monthIndex).toBe(1);
    expect(next.leap.extraDays).toBe("1");
  });
});

describe("list editing", () => {
  it("moveItem moves within bounds and ignores moves past the ends", () => {
    expect(moveItem([1, 2, 3], 0, 1)).toEqual([2, 1, 3]);
    expect(moveItem([1, 2, 3], 2, -1)).toEqual([1, 3, 2]);
    expect(moveItem([1, 2, 3], 0, -1)).toEqual([1, 2, 3]);
    expect(moveItem([1, 2, 3], 2, 1)).toEqual([1, 2, 3]);
  });

  it("moves months and keeps the leap month attached to its month", () => {
    const draft = draftFromCalendar("x", ELVEN.definition); // leap month = Hesperin (index 1)
    const up = moveMonth(draft, 1, -1);
    expect(up.months.map((m) => m.name)).toEqual(["Hesperin", "Calith", "Lorien"]);
    expect(up.leap.monthIndex).toBe(0);
    const swapped = moveMonth(draft, 0, 1); // Calith moves below Hesperin
    expect(swapped.months.map((m) => m.name)).toEqual(["Hesperin", "Calith", "Lorien"]);
    expect(swapped.leap.monthIndex).toBe(0);
    expect(moveMonth(draft, 0, -1)).toBe(draft);
  });

  it("adds a month copying the last length and removes months fixing the leap index", () => {
    const draft = draftFromCalendar("x", ELVEN.definition);
    const added = addMonth(draft);
    expect(added.months.at(-1)).toMatchObject({ name: "Month 4", days: "40" });

    expect(removeMonth(draft, 0).leap.monthIndex).toBe(0); // leap month shifts up
    expect(removeMonth(draft, 2).leap.monthIndex).toBe(1);
    expect(removeMonth(removeMonth(draft, 2), 1).leap.monthIndex).toBe(0); // clamped
  });

  it("names copies uniquely", () => {
    expect(uniqueCopyName("Standard", ["Standard"])).toBe("Standard (copy)");
    expect(uniqueCopyName("Standard", ["Standard", "standard (copy)"])).toBe("Standard (copy 2)");
  });

  it("ignores row keys when comparing drafts", () => {
    const a = draftFromCalendar("x", ELVEN.definition);
    const b = draftFromCalendar("x", ELVEN.definition);
    expect(a.months[0]?.key).not.toBe(b.months[0]?.key);
    expect(draftSignature(a)).toBe(draftSignature(b));
    b.months[0] = { ...b.months[0]!, days: "41" };
    expect(draftSignature(a)).not.toBe(draftSignature(b));
  });
});

describe("yearLengths", () => {
  it("reports normal and leap year length", () => {
    expect(yearLengths(STANDARD.definition)).toEqual({ normal: 360, leap: null });
    expect(yearLengths(ELVEN.definition)).toEqual({ normal: 120, leap: 121 });
    const onlyRemoval: CalendarDefinition = {
      ...ELVEN.definition,
      leap: { monthIndex: 0, extraDays: 1, rules: [{ every: 4, add: false }] },
    };
    expect(yearLengths(onlyRemoval).leap).toBeNull();
  });
});
