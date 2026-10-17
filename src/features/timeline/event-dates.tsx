import { useEffect, useState } from "react";
import { CalendarSearch } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useAutosave, type SaveStatus } from "@/features/entities/use-autosave";
import type { CalendarDefinition, EventSummary } from "@/lib/bindings";
import { DateEditor } from "./date-editor";
import { useDefaultCalendar, useEvent, useUpdateEventDates } from "./queries";
import { useOpenTimeline } from "./use-open-timeline";

const IMPORTANCE_LABELS = ["Minor", "Low", "Normal", "High", "Major"];

interface Props {
  entityId: string;
  onStatusChange?: (status: SaveStatus, error: string | null) => void;
}

/** "When" section of an event page: start date, optional end date, importance. */
export function EventDates({ entityId, onStatusChange }: Props) {
  const event = useEvent(entityId);
  const calendar = useDefaultCalendar();

  if (event.isPending || calendar.isPending) {
    return <p className="text-sm text-muted-foreground">Loading dates…</p>;
  }
  if (event.error || !event.data || !calendar.data) {
    return (
      <p className="text-sm text-destructive">
        {event.error?.message ?? calendar.error?.message ?? "Could not load the event dates."}
      </p>
    );
  }
  return (
    <EventDatesForm
      key={entityId}
      event={event.data}
      def={calendar.data.definition}
      onStatusChange={onStatusChange}
    />
  );
}

interface Draft {
  startDay: number;
  endDay: number | null;
  importance: number;
}

function EventDatesForm({
  event,
  def,
  onStatusChange,
}: {
  event: EventSummary;
  def: CalendarDefinition;
  onStatusChange: Props["onStatusChange"];
}) {
  const updateDates = useUpdateEventDates();
  const openTimeline = useOpenTimeline();
  const [draft, setDraft] = useState<Draft>({
    startDay: event.startDay,
    endDay: event.endDay,
    importance: event.importance,
  });

  // Bumped when the checkbox re-seeds the end date, so the end editor remounts with it.
  const [endEditorKey, setEndEditorKey] = useState(0);

  const rangeError =
    draft.endDay !== null && draft.endDay < draft.startDay
      ? "The end date must not be before the start date."
      : null;

  const { status, error } = useAutosave(draft, async (value) => {
    // Invalid ranges stay on screen (with the message below) but are never sent.
    if (value.endDay !== null && value.endDay < value.startDay) return;
    await updateDates.mutateAsync({ entityId: event.entityId, ...value });
  });
  useEffect(() => {
    onStatusChange?.(status, error);
  }, [status, error, onStatusChange]);

  return (
    <section aria-label="When" className="flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-medium text-muted-foreground">When</h2>
        <Button
          variant="ghost"
          size="sm"
          onClick={() =>
            openTimeline({ startDay: draft.startDay, endDay: draft.endDay ?? draft.startDay })
          }
        >
          <CalendarSearch /> Show on timeline
        </Button>
      </div>

      <DateEditor
        def={def}
        day={draft.startDay}
        label="Starts"
        onChange={(startDay) => setDraft((d) => ({ ...d, startDay }))}
      />

      <label className="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={draft.endDay !== null}
          onChange={(e) => {
            const checked = e.target.checked;
            setEndEditorKey((k) => k + 1);
            setDraft((d) => ({ ...d, endDay: checked ? d.startDay : null }));
          }}
        />
        Lasts more than a day (has an end date)
      </label>

      {draft.endDay !== null && (
        <DateEditor
          key={endEditorKey}
          def={def}
          day={draft.endDay}
          label="Ends"
          onChange={(endDay) => setDraft((d) => ({ ...d, endDay }))}
        />
      )}
      {rangeError && <p className="text-sm text-destructive">{rangeError}</p>}

      <div className="flex w-56 flex-col gap-2">
        <Label htmlFor="event-importance">Importance</Label>
        <Select
          value={String(draft.importance)}
          onValueChange={(v) => setDraft((d) => ({ ...d, importance: Number(v) }))}
        >
          <SelectTrigger id="event-importance" className="w-full">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {IMPORTANCE_LABELS.map((name, i) => (
              <SelectItem key={i} value={String(i + 1)}>
                {i + 1} - {name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
    </section>
  );
}
