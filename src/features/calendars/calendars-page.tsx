import { useState } from "react";
import { useBlocker } from "@tanstack/react-router";
import { CalendarEditor } from "./calendar-editor";
import { CalendarsList } from "./calendars-list";
import { useCalendars } from "./queries";

const LEAVE_MESSAGE = "You have unsaved calendar changes. Leave without saving?";

/** Route component for `/world/calendars`: the calendar list plus an editor for one calendar. */
export function CalendarsPage() {
  const calendars = useCalendars();
  // undefined = nothing chosen yet (open the default), null = a new calendar.
  const [selected, setSelected] = useState<string | null | undefined>(undefined);
  const [dirty, setDirty] = useState(false);

  // Leaving the page by any route (sidebar, palette, back) asks first.
  useBlocker({
    shouldBlockFn: () => !window.confirm(LEAVE_MESSAGE),
    enableBeforeUnload: () => dirty,
    disabled: !dirty,
  });

  if (calendars.isPending) {
    return <p className="p-6 text-sm text-muted-foreground">Loading…</p>;
  }
  if (calendars.error || !calendars.data) {
    return (
      <p className="p-6 text-sm text-destructive">
        {calendars.error?.message ?? "Could not load the calendars."}
      </p>
    );
  }

  const list = calendars.data;
  const selectedId =
    selected === undefined ? (list.find((c) => c.isDefault)?.id ?? list[0]?.id ?? null) : selected;
  const current = list.find((c) => c.id === selectedId) ?? null;

  /** Switches what the editor shows, asking first when that would drop unsaved edits. */
  function choose(next: string | null) {
    if (next === selectedId) return;
    if (dirty && !window.confirm(LEAVE_MESSAGE)) return;
    setDirty(false);
    setSelected(next);
  }

  return (
    <div className="min-w-0 flex-1 overflow-auto">
      <div className="mx-auto flex max-w-6xl flex-col gap-6 p-6">
        <header>
          <h1 className="text-xl font-semibold">Calendars</h1>
          <p className="mt-1 text-sm text-muted-foreground">
            Every calendar describes the same moments in its own words. Editing a calendar only
            changes how dates are shown; events never move.
          </p>
        </header>
        <div className="grid gap-6 lg:grid-cols-[18rem_minmax(0,1fr)]">
          <CalendarsList
            calendars={list}
            selectedId={selectedId}
            onSelect={choose}
            onNew={() => choose(null)}
            onDeleted={(id) => {
              if (id === selectedId) {
                setDirty(false);
                setSelected(undefined);
              }
            }}
            onDuplicated={(c) => choose(c.id)}
          />
          <section aria-label="Calendar editor" className="min-w-0">
            <h2 className="mb-3 text-sm font-semibold">
              {current ? `Editing ${current.name}` : "New calendar"}
            </h2>
            <CalendarEditor
              key={current?.id ?? "new"}
              calendar={current}
              onDirtyChange={setDirty}
              onSaved={(saved) => {
                setDirty(false);
                setSelected(saved.id);
              }}
            />
          </section>
        </div>
      </div>
    </div>
  );
}
