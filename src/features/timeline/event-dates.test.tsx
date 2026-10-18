import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ELVEN, STANDARD } from "@/features/calendars/test-calendars";
import { renderWithQuery } from "@/test/render";
import { fromDate } from "./calendar";
import { EventDates } from "./event-dates";

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const commands = vi.hoisted(() => ({
  getEvent: vi.fn(),
  listCalendars: vi.fn(),
  getDisplayCalendar: vi.fn(),
  updateEventDates: vi.fn(),
}));
vi.mock("@/lib/bindings", () => ({ commands }));
vi.mock("@/features/settings/queries", () => ({
  useSettingsValue: () => ({ autosaveDelayMs: 10 }),
}));
vi.mock("./use-open-timeline", () => ({ useOpenTimeline: () => vi.fn() }));

const START = fromDate(STANDARD.definition, 0, 3, 5, 12);
const EVENT = {
  entityId: "e1",
  name: "The Founding",
  startDay: START,
  endDay: null,
  importance: 3,
  calendarId: STANDARD.id,
};

describe("EventDates", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    commands.getEvent.mockResolvedValue(ok(EVENT));
    commands.listCalendars.mockResolvedValue(ok([STANDARD, ELVEN]));
    commands.getDisplayCalendar.mockResolvedValue(ok(STANDARD));
    commands.updateEventDates.mockImplementation(
      (
        _id: string,
        startDay: number,
        endDay: number | null,
        importance: number,
        calendarId: string,
      ) => Promise.resolve(ok({ ...EVENT, startDay, endDay, importance, calendarId })),
    );
  });

  it("starts in the display calendar and shows the date in the others", async () => {
    renderWithQuery(<EventDates entityId="e1" />);
    expect(await screen.findByLabelText("Calendar")).toHaveValue(STANDARD.id);
    expect(screen.getByRole("list", { name: "Same date in other calendars" })).toHaveTextContent(
      "(Elven Reckoning)",
    );
  });

  it("saves a date entered in another calendar with that calendar's id", async () => {
    renderWithQuery(<EventDates entityId="e1" />);
    fireEvent.change(await screen.findByLabelText("Calendar"), { target: { value: ELVEN.id } });

    // Pick Elven year 7, Lorien 5, in the first era (the day starts out in the second).
    fireEvent.change(screen.getByLabelText("Era"), { target: { value: "0" } });
    fireEvent.change(screen.getByLabelText("Year"), { target: { value: "7" } });
    fireEvent.change(screen.getByLabelText("Month"), { target: { value: "3" } });
    fireEvent.change(screen.getByLabelText("Day"), { target: { value: "5" } });

    const expected = fromDate(ELVEN.definition, 0, 7, 3, 5);
    await waitFor(() =>
      expect(commands.updateEventDates).toHaveBeenLastCalledWith("e1", expected, null, 3, ELVEN.id),
    );
  });
});
