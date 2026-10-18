import { useState } from "react";
import { Copy, Pencil, Plus, Star, Trash2 } from "lucide-react";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { Calendar } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { uniqueCopyName } from "./draft";
import { useDeleteCalendar, useDuplicateCalendar, useSetDefaultCalendar } from "./queries";

interface Props {
  calendars: Calendar[];
  /** Id of the calendar open in the editor (null while creating a new one). */
  selectedId: string | null;
  onSelect: (id: string) => void;
  onNew: () => void;
  /** Called after a calendar was deleted, with its id. */
  onDeleted: (id: string) => void;
  /** Called after a duplicate was created, so it can be opened. */
  onDuplicated: (calendar: Calendar) => void;
}

/** Every calendar of the world, with set-default, duplicate and delete. */
export function CalendarsList({
  calendars,
  selectedId,
  onSelect,
  onNew,
  onDeleted,
  onDuplicated,
}: Props) {
  const setDefault = useSetDefaultCalendar();
  const duplicate = useDuplicateCalendar();
  const remove = useDeleteCalendar();
  const [confirmDelete, setConfirmDelete] = useState<Calendar | null>(null);

  const error = setDefault.error ?? duplicate.error ?? remove.error;

  return (
    <section aria-label="Calendars" className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold">Your calendars</h2>
        <Button variant="outline" size="sm" onClick={onNew}>
          <Plus /> New calendar
        </Button>
      </div>
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error.message}
        </p>
      )}
      <ul className="flex flex-col gap-2">
        {calendars.map((c) => (
          <li
            key={c.id}
            className={cn(
              "flex flex-col gap-2 rounded-lg border p-3",
              c.id === selectedId && "border-ring bg-accent/40",
            )}
          >
            <div className="flex items-center gap-2">
              <span className="truncate font-medium">{c.name}</span>
              {c.isDefault && <Badge variant="secondary">Default</Badge>}
            </div>
            <p className="text-xs text-muted-foreground">
              {c.definition.months.length} months &middot; {c.definition.eras.length}{" "}
              {c.definition.eras.length === 1 ? "era" : "eras"}
              {c.definition.leap ? " · leap years" : ""}
            </p>
            <div className="flex flex-wrap gap-1">
              <Button
                variant="ghost"
                size="sm"
                aria-label={`Edit ${c.name}`}
                onClick={() => onSelect(c.id)}
              >
                <Pencil /> Edit
              </Button>
              <Button
                variant="ghost"
                size="sm"
                aria-label={`Make ${c.name} the default`}
                disabled={c.isDefault || setDefault.isPending}
                onClick={() => setDefault.mutate(c.id)}
              >
                <Star /> Make default
              </Button>
              <Button
                variant="ghost"
                size="sm"
                aria-label={`Duplicate ${c.name}`}
                disabled={duplicate.isPending}
                onClick={() =>
                  duplicate.mutate(
                    {
                      id: c.id,
                      newName: uniqueCopyName(
                        c.name,
                        calendars.map((x) => x.name),
                      ),
                    },
                    { onSuccess: onDuplicated },
                  )
                }
              >
                <Copy /> Duplicate
              </Button>
              <Button
                variant="ghost"
                size="sm"
                aria-label={`Delete ${c.name}`}
                disabled={c.isDefault}
                title={c.isDefault ? "Make another calendar the default first" : undefined}
                onClick={() => setConfirmDelete(c)}
              >
                <Trash2 /> Delete
              </Button>
            </div>
          </li>
        ))}
      </ul>

      <AlertDialog
        open={confirmDelete !== null}
        onOpenChange={(open) => {
          if (!open) setConfirmDelete(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete calendar “{confirmDelete?.name}”?</AlertDialogTitle>
            <AlertDialogDescription>
              Events keep their dates; they are simply shown in the default calendar from now on.
              This cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                if (confirmDelete) {
                  const id = confirmDelete.id;
                  remove.mutate(id, { onSuccess: () => onDeleted(id) });
                }
                setConfirmDelete(null);
              }}
            >
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </section>
  );
}
