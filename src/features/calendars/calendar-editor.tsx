import { useEffect, useMemo, useRef, useState } from "react";
import { Save } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useTimelineStore } from "@/features/timeline/timeline-store";
import type { Calendar, CalendarDefinition, ValidationIssue } from "@/lib/bindings";
import { useDebouncedValue } from "@/lib/use-debounced-value";
import { CommandError } from "@/lib/unwrap";
import {
  draftFromCalendar,
  draftSignature,
  issuesByField,
  newDraft,
  parseDraft,
  parseSaveError,
  yearLengths,
  type CalendarDraft,
} from "./draft";
import {
  EpochSection,
  ErasSection,
  LeapSection,
  MonthsSection,
  WeekdaysSection,
} from "./editor-sections";
import { usePreviewCalendar, useCreateCalendar, useUpdateCalendar } from "./queries";

/** How long typing must pause before the preview asks the backend again. */
const PREVIEW_DELAY_MS = 300;
/** Days after the timeline centre that the preview formats, to show spaced-out dates. */
const SAMPLE_OFFSETS = [1, 45, 400, 5000];

interface Props {
  /** The calendar to edit, or null to create a new one. */
  calendar: Calendar | null;
  onSaved: (calendar: Calendar) => void;
  /** Reports whether there are unsaved changes (for the leave-page guard). */
  onDirtyChange: (dirty: boolean) => void;
}

/** Form for one calendar with a live preview of the unsaved definition. */
export function CalendarEditor({ calendar, onSaved, onDirtyChange }: Props) {
  const create = useCreateCalendar();
  const update = useUpdateCalendar();

  const [initial, setInitial] = useState<CalendarDraft>(() =>
    calendar ? draftFromCalendar(calendar.name, calendar.definition) : newDraft(),
  );
  const [draft, setDraft] = useState<CalendarDraft>(initial);
  const [saveIssues, setSaveIssues] = useState<ValidationIssue[]>([]);
  const [saved, setSaved] = useState(false);

  const dirty = draftSignature(draft) !== draftSignature(initial);
  const onDirtyRef = useRef(onDirtyChange);
  useEffect(() => {
    onDirtyRef.current = onDirtyChange;
  });
  useEffect(() => {
    onDirtyRef.current(dirty);
  }, [dirty]);
  // Leaving the editor (another calendar, another page) clears the flag.
  useEffect(() => () => onDirtyRef.current(false), []);

  const parsed = useMemo(() => parseDraft(draft), [draft]);

  // --- live preview ---------------------------------------------------------
  const centreDay = Math.floor(useTimelineStore((s) => s.viewport.centerDay));
  const debouncedDefinition = useDebouncedValue(parsed.definition, PREVIEW_DELAY_MS);
  const eraCount = debouncedDefinition?.eras.length ?? 0;
  const sampleDays = useMemo(() => SAMPLE_OFFSETS.map((offset) => centreDay + offset), [centreDay]);
  const previewDays = useMemo(
    () => [
      ...(debouncedDefinition?.eras.map((e) => e.startDay) ?? []),
      0,
      centreDay,
      ...sampleDays,
    ],
    [debouncedDefinition, centreDay, sampleDays],
  );
  const preview = usePreviewCalendar(debouncedDefinition, previewDays);
  const previewIssues = parsed.definition ? (preview.data?.issues ?? []) : [];
  const labels = preview.data && preview.data.issues.length === 0 ? preview.data.labels : [];
  const updating = debouncedDefinition !== parsed.definition || preview.isFetching;

  const errors = issuesByField([...parsed.issues, ...previewIssues, ...saveIssues]);
  // Messages that are not tied to any input (everything else shows beside its field).
  const generalIssues = saveIssues.filter((i) => i.field === "");
  const hasProblems = parsed.issues.length > 0 || previewIssues.length > 0;

  function edit(next: CalendarDraft) {
    setDraft(next);
    setSaveIssues([]);
    setSaved(false);
  }

  async function save() {
    if (!parsed.definition) return;
    const name = draft.name.trim();
    setSaveIssues([]);
    try {
      const result = calendar
        ? await update.mutateAsync({ id: calendar.id, name, definition: parsed.definition })
        : await create.mutateAsync({ name, definition: parsed.definition });
      const fresh = draftFromCalendar(result.name, result.definition);
      setInitial(fresh);
      setDraft(fresh);
      setSaved(true);
      onSaved(result);
    } catch (error) {
      if (error instanceof CommandError && error.kind === "validation") {
        const issues = parseSaveError(error.message);
        setSaveIssues(issues.length > 0 ? issues : [{ field: "", message: error.message }]);
      } else {
        setSaveIssues([
          { field: "", message: error instanceof Error ? error.message : String(error) },
        ]);
      }
    }
  }

  const busy = create.isPending || update.isPending;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-end gap-3">
        <label className="flex min-w-64 flex-1 flex-col gap-1 text-sm font-medium">
          Calendar name
          <Input
            aria-label="Calendar name"
            aria-invalid={errors["name"] ? true : undefined}
            value={draft.name}
            onChange={(e) => edit({ ...draft, name: e.target.value })}
          />
        </label>
        <Button onClick={() => void save()} disabled={busy || !dirty || !parsed.definition}>
          <Save /> {calendar ? "Save changes" : "Create calendar"}
        </Button>
        {saved && !dirty && (
          <span role="status" className="pb-2 text-sm text-muted-foreground">
            Saved.
          </span>
        )}
      </div>
      {errors["name"] && <p className="-mt-2 text-xs text-destructive">{errors["name"]}</p>}
      {generalIssues.map((issue, i) => (
        <p key={i} role="alert" className="text-sm text-destructive">
          {issue.message}
        </p>
      ))}

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_20rem]">
        <div className="flex min-w-0 flex-col gap-4">
          <MonthsSection draft={draft} errors={errors} onChange={edit} />
          <WeekdaysSection draft={draft} errors={errors} onChange={edit} />
          <ErasSection
            draft={draft}
            errors={errors}
            onChange={edit}
            eraDates={labels.slice(0, eraCount)}
          />
          <EpochSection
            draft={draft}
            errors={errors}
            onChange={edit}
            dayZeroLabel={labels[eraCount]}
          />
          <LeapSection draft={draft} errors={errors} onChange={edit} />
        </div>
        <PreviewPanel
          definition={debouncedDefinition}
          labels={labels.slice(eraCount)}
          centreDay={centreDay}
          sampleDays={sampleDays}
          issues={[...parsed.issues, ...previewIssues]}
          hasProblems={hasProblems}
          updating={updating}
        />
      </div>
    </div>
  );
}

function PreviewPanel({
  definition,
  labels,
  centreDay,
  sampleDays,
  issues,
  hasProblems,
  updating,
}: {
  definition: CalendarDefinition | null;
  /** Labels for [day 0, timeline centre, ...sampleDays]. */
  labels: string[];
  centreDay: number;
  sampleDays: number[];
  issues: ValidationIssue[];
  hasProblems: boolean;
  updating: boolean;
}) {
  const lengths = definition ? yearLengths(definition) : null;
  return (
    <aside
      aria-label="Preview"
      aria-busy={updating}
      className="flex h-fit flex-col gap-3 rounded-lg border bg-muted/30 p-4 xl:sticky xl:top-4"
    >
      <h3 className="text-sm font-semibold">Live preview</h3>
      {hasProblems ? (
        <div className="flex flex-col gap-1">
          <p className="text-sm text-destructive">Fix these to see the preview:</p>
          <ul className="list-disc pl-5 text-xs text-destructive">
            {issues.map((issue, i) => (
              <li key={i}>
                {issue.field}: {issue.message}
              </li>
            ))}
          </ul>
        </div>
      ) : (
        <>
          {lengths && (
            <p className="text-sm">
              Normal year: <strong>{lengths.normal} days</strong>
              {lengths.leap !== null && (
                <>
                  {" "}
                  &middot; leap year: <strong>{lengths.leap} days</strong>
                </>
              )}
            </p>
          )}
          {labels.length > 0 ? (
            <dl className="flex flex-col gap-2 text-sm">
              <PreviewRow term="Absolute day 0" value={labels[0]} />
              <PreviewRow
                term={`Timeline centre (day ${centreDay.toLocaleString("en-US")})`}
                value={labels[1]}
              />
              {sampleDays.map((day, i) => (
                <PreviewRow
                  key={day}
                  term={`${SAMPLE_OFFSETS[i]?.toLocaleString("en-US")} days later`}
                  value={labels[i + 2]}
                />
              ))}
            </dl>
          ) : (
            <p className="text-xs text-muted-foreground">Checking…</p>
          )}
        </>
      )}
      <p className="text-xs text-muted-foreground">
        Dates are the same moments as in your other calendars; only their names change.
      </p>
    </aside>
  );
}

function PreviewRow({ term, value }: { term: string; value: string | undefined }) {
  return (
    <div>
      <dt className="text-xs text-muted-foreground">{term}</dt>
      <dd>{value ?? "…"}</dd>
    </div>
  );
}
