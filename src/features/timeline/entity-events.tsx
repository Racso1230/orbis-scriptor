import { CalendarDays } from "lucide-react";
import { useOpenEntity } from "@/features/entities/use-open-entity";
import { useDisplayCalendar } from "@/features/calendars/queries";
import { formatDate } from "./calendar";
import { useEventsForEntity } from "./queries";

/** "Events" section on non-event pages. Renders nothing when the entity has none. */
export function EntityEvents({ entityId }: { entityId: string }) {
  const events = useEventsForEntity(entityId);
  const calendar = useDisplayCalendar();
  const open = useOpenEntity();

  if (!events.data || events.data.length === 0) return null;
  const def = calendar.data?.definition;

  return (
    <section aria-label="Events" className="flex flex-col gap-2">
      <h2 className="text-sm font-medium text-muted-foreground">Events ({events.data.length})</h2>
      <ul className="flex flex-col gap-1">
        {events.data.map((e) => (
          <li key={e.entityId}>
            <button
              type="button"
              onClick={() => open(e.entityId)}
              className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm hover:bg-accent"
            >
              <CalendarDays className="size-4 shrink-0 text-muted-foreground" />
              <span className="truncate">{e.name}</span>
              {def && (
                <span className="ml-auto shrink-0 text-xs text-muted-foreground">
                  {formatDate(def, e.startDay, "full")}
                  {e.endDay !== null && ` – ${formatDate(def, e.endDay, "full")}`}
                </span>
              )}
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
