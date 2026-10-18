import type { Calendar } from "@/lib/bindings";

// Calendars shared by component tests (the bindings are mocked there).

/** The default calendar: 12 months of 30 days, one era. */
export const STANDARD: Calendar = {
  id: "cal-standard",
  name: "Standard",
  isDefault: true,
  definition: {
    months: Array.from({ length: 12 }, (_, i) => ({ name: `Month ${i + 1}`, days: 30 })),
    weekdays: [],
    eras: [{ name: "After Founding", abbreviation: "AF", startDay: 0 }],
    epochOffset: 0,
    leap: null,
  },
};

/** A leap-year calendar with two eras, like the sample world's "Elven Reckoning" in miniature. */
export const ELVEN: Calendar = {
  id: "cal-elven",
  name: "Elven Reckoning",
  isDefault: false,
  definition: {
    months: [
      { name: "Calith", days: 40 },
      { name: "Hesperin", days: 40 },
      { name: "Lorien", days: 40 },
    ],
    weekdays: ["Lumen", "Sylva"],
    eras: [
      { name: "Age of Starlight", abbreviation: "AS", startDay: -1000 },
      { name: "Age of Embers", abbreviation: "AE", startDay: 500 },
    ],
    epochOffset: -1000,
    leap: { monthIndex: 1, extraDays: 1, rules: [{ every: 4, add: true }] },
  },
};
