import { useId, useState } from "react";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { NativeSelect } from "@/components/native-select";
import type { Calendar, CalendarDefinition } from "@/lib/bindings";
import { daysInMonth, eraLabel, formatDate, fromDate, toDate, yearIndexFor } from "./calendar";

interface Props {
  /** Every calendar of the world; the editor can enter the date in any of them. */
  calendars: Calendar[];
  /** The calendar the fields are in. */
  calendarId: string;
  /** Absolute day number (shared by all calendars). */
  day: number;
  label: string;
  onChange: (day: number) => void;
  /** When given, a calendar select is shown and reports the new choice. */
  onCalendarChange?: (calendarId: string) => void;
}

/**
 * Edits one absolute day as era / year / month / day in the chosen calendar,
 * and shows the same moment in the other calendars as read-only hints.
 */
export function DateEditor(props: Props) {
  // Remount when the calendar changes so the fields are re-derived from the
  // same absolute day in the new calendar.
  const calendar = props.calendars.find((c) => c.id === props.calendarId) ?? props.calendars[0];
  if (!calendar) return null;
  return <DateFields key={calendar.id} {...props} calendar={calendar} />;
}

/** How many days the month has in the year typed so far (leap days included). */
function monthLengthFor(
  def: CalendarDefinition,
  eraIndex: number,
  yearText: string,
  month: number,
): number {
  const year = Number(yearText);
  if (yearText.trim() === "" || !Number.isInteger(year)) {
    // No usable year yet: allow the longest this month can ever be.
    const leapExtra = def.leap?.monthIndex === month - 1 ? def.leap.extraDays : 0;
    return (def.months[month - 1]?.days ?? 1) + leapExtra;
  }
  return daysInMonth(def, yearIndexFor(def, eraIndex, year), month);
}

function DateFields({
  calendars,
  calendar,
  day,
  label,
  onChange,
  onCalendarChange,
}: Props & { calendar: Calendar }) {
  const id = useId();
  const def = calendar.definition;
  const initial = toDate(def, day);
  const [eraIndex, setEraIndex] = useState(initial.eraIndex);
  const [year, setYear] = useState(String(initial.year));
  const [month, setMonth] = useState(initial.month);
  const [dayText, setDayText] = useState(String(initial.day));

  const monthLength = monthLengthFor(def, eraIndex, year, month);

  function commit(next: { eraIndex: number; year: string; month: number; dayText: string }) {
    const y = Number(next.year);
    const d = Number(next.dayText);
    if (next.year.trim() === "" || !Number.isInteger(y)) return;
    const length = daysInMonth(def, yearIndexFor(def, next.eraIndex, y), next.month);
    if (next.dayText.trim() === "" || !Number.isInteger(d) || d < 1 || d > length) return;
    onChange(fromDate(def, next.eraIndex, y, next.month, d));
  }

  const current = { eraIndex, year, month, dayText };
  const dayInvalid =
    dayText.trim() === "" ||
    !Number.isInteger(Number(dayText)) ||
    Number(dayText) < 1 ||
    Number(dayText) > monthLength;

  const others = calendars.filter((c) => c.id !== calendar.id);

  return (
    <fieldset className="flex flex-col gap-2">
      <legend className="mb-2 text-sm font-medium">{label}</legend>
      <div className="flex flex-wrap items-end gap-3">
        <div className="flex w-20 flex-col gap-1">
          <Label htmlFor={`${id}-day`} className="text-xs text-muted-foreground">
            Day
          </Label>
          <Input
            id={`${id}-day`}
            type="number"
            min={1}
            max={monthLength}
            value={dayText}
            aria-invalid={dayInvalid}
            onChange={(e) => {
              setDayText(e.target.value);
              commit({ ...current, dayText: e.target.value });
            }}
          />
        </div>
        <div className="flex w-44 flex-col gap-1">
          <Label htmlFor={`${id}-month`} className="text-xs text-muted-foreground">
            Month
          </Label>
          <NativeSelect
            id={`${id}-month`}
            value={String(month)}
            onChange={(e) => {
              const m = Number(e.target.value);
              setMonth(m);
              commit({ ...current, month: m });
            }}
          >
            {def.months.map((m, i) => (
              <option key={i} value={String(i + 1)}>
                {m.name}
              </option>
            ))}
          </NativeSelect>
        </div>
        <div className="flex w-28 flex-col gap-1">
          <Label htmlFor={`${id}-year`} className="text-xs text-muted-foreground">
            Year
          </Label>
          <Input
            id={`${id}-year`}
            type="number"
            value={year}
            onChange={(e) => {
              setYear(e.target.value);
              commit({ ...current, year: e.target.value });
            }}
          />
        </div>
        {def.eras.length > 1 && (
          <div className="flex w-40 flex-col gap-1">
            <Label htmlFor={`${id}-era`} className="text-xs text-muted-foreground">
              Era
            </Label>
            <NativeSelect
              id={`${id}-era`}
              value={String(eraIndex)}
              onChange={(e) => {
                const era = Number(e.target.value);
                setEraIndex(era);
                commit({ ...current, eraIndex: era });
              }}
            >
              {def.eras.map((era, i) => (
                <option key={i} value={String(i)}>
                  {era.name}
                </option>
              ))}
            </NativeSelect>
          </div>
        )}
        {def.eras.length === 1 && (
          <span className="pb-2 text-sm text-muted-foreground">{eraLabel(def, 0)}</span>
        )}
        {onCalendarChange && calendars.length > 1 && (
          <div className="flex w-48 flex-col gap-1">
            <Label htmlFor={`${id}-calendar`} className="text-xs text-muted-foreground">
              Calendar
            </Label>
            <NativeSelect
              id={`${id}-calendar`}
              value={calendar.id}
              onChange={(e) => onCalendarChange(e.target.value)}
            >
              {calendars.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </NativeSelect>
          </div>
        )}
      </div>
      {dayInvalid && (
        <p className="text-xs text-destructive">Day must be between 1 and {monthLength}.</p>
      )}
      {others.length > 0 && (
        <ul aria-label="Same date in other calendars" className="text-xs text-muted-foreground">
          {others.map((c) => (
            <li key={c.id}>
              = {formatDate(c.definition, day, "full")} ({c.name})
            </li>
          ))}
        </ul>
      )}
    </fieldset>
  );
}
