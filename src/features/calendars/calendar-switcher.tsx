import { NativeSelect } from "@/components/native-select";
import { useCalendars, useDisplayCalendar, useSetDisplayCalendar } from "./queries";

/** Chooses the calendar every date is shown in (stored in the world). */
export function CalendarSwitcher() {
  const calendars = useCalendars();
  const display = useDisplayCalendar();
  const setDisplay = useSetDisplayCalendar();

  if (!calendars.data || !display.data) return null;

  return (
    <NativeSelect
      aria-label="Display calendar"
      className="max-w-48"
      value={display.data.id}
      onChange={(e) => {
        setDisplay.mutate(e.target.value);
      }}
    >
      {calendars.data.map((c) => (
        <option key={c.id} value={c.id}>
          {c.name}
        </option>
      ))}
    </NativeSelect>
  );
}
