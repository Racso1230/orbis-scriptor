import { useState } from "react";
import { useBlocker } from "@tanstack/react-router";
import { useEntities, useEntityTypes } from "@/features/entities/queries";
import { TypeEditor } from "./type-editor";
import { TypesList } from "./types-list";

const LEAVE_MESSAGE = "You have unsaved type changes. Leave without saving?";

/** Route component for `/world/types`: the type list plus an editor for one type. */
export function TypesPage() {
  const types = useEntityTypes();
  const entities = useEntities();
  // undefined = nothing chosen yet, null = a new type, otherwise a type key.
  const [selected, setSelected] = useState<string | null | undefined>(undefined);
  const [dirty, setDirty] = useState(false);

  // Leaving the page by any route (sidebar, palette, back) asks first.
  useBlocker({
    shouldBlockFn: () => !window.confirm(LEAVE_MESSAGE),
    enableBeforeUnload: () => dirty,
    disabled: !dirty,
  });

  if (types.isPending) return <p className="p-6 text-sm text-muted-foreground">Loading…</p>;
  if (types.error || !types.data) {
    return (
      <p className="p-6 text-sm text-destructive">
        {types.error?.message ?? "Could not load the entity types."}
      </p>
    );
  }

  const list = types.data;
  const current =
    typeof selected === "string" ? (list.find((t) => t.key === selected) ?? null) : null;

  /** Switches what the editor shows, asking first when that would drop unsaved edits. */
  function choose(next: string | null) {
    if (next === selected) return;
    if (dirty && !window.confirm(LEAVE_MESSAGE)) return;
    setDirty(false);
    setSelected(next);
  }

  return (
    <div className="min-w-0 flex-1 overflow-auto">
      <div className="mx-auto flex max-w-6xl flex-col gap-6 p-6">
        <header>
          <h1 className="text-xl font-semibold">Entity types</h1>
          <p className="mt-1 text-sm text-muted-foreground">
            Add your own kinds of entity with their own fields, or give the built-in types extra
            fields. Removing a field never deletes the values already stored.
          </p>
        </header>
        <div className="grid gap-6 lg:grid-cols-[20rem_minmax(0,1fr)]">
          <TypesList
            types={list}
            entities={entities.data ?? []}
            selectedKey={typeof selected === "string" ? selected : null}
            onSelect={choose}
            onNew={() => choose(null)}
            onDeleted={(key) => {
              if (key === selected) {
                setDirty(false);
                setSelected(undefined);
              }
            }}
          />
          <section aria-label="Type editor" className="min-w-0">
            {selected === undefined ? (
              <p className="text-sm text-muted-foreground">
                Choose a type to edit, or create a new one.
              </p>
            ) : (
              <>
                <h2 className="mb-3 text-sm font-semibold">
                  {current ? `Editing ${current.label}` : "New type"}
                </h2>
                <TypeEditor
                  key={current?.key ?? "new"}
                  type={current}
                  types={list}
                  onDirtyChange={setDirty}
                  onSaved={(saved) => {
                    setDirty(false);
                    setSelected(saved.key);
                  }}
                />
              </>
            )}
          </section>
        </div>
      </div>
    </div>
  );
}
