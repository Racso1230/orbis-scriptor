import { useNavigate } from "@tanstack/react-router";
import { NativeSelect } from "@/components/native-select";
import { useCalendars, useDisplayCalendar, useSetDisplayCalendar } from "./queries";

/** Value of the "Manage calendars…" option; it navigates instead of choosing a calendar. */
const MANAGE = "__manage__";

/**
 * Chooses the calendar every date is shown in (stored in the world). The last
 * option opens the calendar editor.
 */
export function CalendarSwitcher() {
  const navigate = useNavigate();
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
        if (e.target.value === MANAGE) {
          void navigate({ to: "/world/calendars" });
        } else {
          setDisplay.mutate(e.target.value);
        }
      }}
    >
      {calendars.data.map((c) => (
        <option key={c.id} value={c.id}>
          {c.name}
        </option>
      ))}
      <option value={MANAGE}>Manage calendars…</option>
    </NativeSelect>
  );
}
