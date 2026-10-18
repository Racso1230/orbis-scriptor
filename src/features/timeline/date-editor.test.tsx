import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ELVEN, STANDARD } from "@/features/calendars/test-calendars";
import { formatDate, fromDate, toDate } from "./calendar";
import { DateEditor } from "./date-editor";

const CALENDARS = [STANDARD, ELVEN];

/** Holds the calendar and day like EventDates does, and reports every committed day. */
function Harness({ startDay, onDay }: { startDay: number; onDay: (day: number) => void }) {
  const [calendarId, setCalendarId] = useState(STANDARD.id);
  const [day, setDay] = useState(startDay);
  return (
    <DateEditor
      calendars={CALENDARS}
      calendarId={calendarId}
      onCalendarChange={setCalendarId}
      day={day}
      label="Starts"
      onChange={(d) => {
        setDay(d);
        onDay(d);
      }}
    />
  );
}

const inputValue = (name: string) => (screen.getByLabelText(name) as HTMLInputElement).value;

describe("DateEditor", () => {
  const day = fromDate(STANDARD.definition, 0, 3, 5, 12);

  it("shows the date in the chosen calendar and the same moment in the others", () => {
    render(<Harness startDay={day} onDay={vi.fn()} />);
    expect(inputValue("Day")).toBe("12");
    expect(inputValue("Year")).toBe("3");
    expect(screen.getByLabelText("Month")).toHaveValue("5");

    const hints = screen.getByRole("list", { name: "Same date in other calendars" });
    expect(hints).toHaveTextContent(
      `= ${formatDate(ELVEN.definition, day, "full")} (Elven Reckoning)`,
    );
    expect(hints).not.toHaveTextContent("Standard");
  });

  it("re-derives the fields when the calendar changes, keeping the same absolute day", () => {
    const onDay = vi.fn();
    render(<Harness startDay={day} onDay={onDay} />);
    // The standard calendar has one era, so there is no era select yet.
    expect(screen.queryByLabelText("Era")).toBeNull();

    fireEvent.change(screen.getByLabelText("Calendar"), { target: { value: ELVEN.id } });

    const elven = toDate(ELVEN.definition, day);
    expect(inputValue("Day")).toBe(String(elven.day));
    expect(inputValue("Year")).toBe(String(elven.year));
    expect(screen.getByLabelText("Month")).toHaveValue(String(elven.month));
    expect(screen.getByLabelText("Era")).toHaveValue(String(elven.eraIndex));
    expect(screen.getByRole("option", { name: "Hesperin" })).toBeInTheDocument();
    // Switching only re-displays the day; nothing is reported as a change.
    expect(onDay).not.toHaveBeenCalled();
    // And the hint now lists the standard calendar.
    expect(screen.getByRole("list", { name: "Same date in other calendars" })).toHaveTextContent(
      `= ${formatDate(STANDARD.definition, day, "full")} (Standard)`,
    );
  });

  it("allows the leap day only in a leap year", () => {
    const onDay = vi.fn();
    render(<Harness startDay={fromDate(ELVEN.definition, 0, 3, 2, 1)} onDay={onDay} />);
    fireEvent.change(screen.getByLabelText("Calendar"), { target: { value: ELVEN.id } });
    expect(screen.getByLabelText("Month")).toHaveValue("2");

    // Year 3 is a normal year: Hesperin has 40 days.
    expect(screen.getByLabelText("Day")).toHaveAttribute("max", "40");
    fireEvent.change(screen.getByLabelText("Day"), { target: { value: "41" } });
    expect(screen.getByText("Day must be between 1 and 40.")).toBeInTheDocument();
    expect(onDay).not.toHaveBeenCalled();

    // Year 4 is a leap year: day 41 exists and is committed.
    fireEvent.change(screen.getByLabelText("Year"), { target: { value: "4" } });
    expect(screen.getByLabelText("Day")).toHaveAttribute("max", "41");
    expect(screen.queryByText(/Day must be between/)).toBeNull();
    expect(onDay).toHaveBeenLastCalledWith(fromDate(ELVEN.definition, 0, 4, 2, 41));
    expect(toDate(ELVEN.definition, fromDate(ELVEN.definition, 0, 4, 2, 41))).toMatchObject({
      month: 2,
      day: 41,
      year: 4,
    });
  });

  it("reports a day entered in another calendar as the absolute day number", () => {
    const onDay = vi.fn();
    render(<Harness startDay={day} onDay={onDay} />);
    fireEvent.change(screen.getByLabelText("Calendar"), { target: { value: ELVEN.id } });
    fireEvent.change(screen.getByLabelText("Year"), { target: { value: "280" } });
    expect(onDay).toHaveBeenLastCalledWith(
      fromDate(
        ELVEN.definition,
        toDate(ELVEN.definition, day).eraIndex,
        280,
        toDate(ELVEN.definition, day).month,
        toDate(ELVEN.definition, day).day,
      ),
    );
  });
});
