import { useState } from "react";
import { ArrowDown, ArrowUp, Pencil, Plus, Trash2 } from "lucide-react";
import { NativeSelect } from "@/components/native-select";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { EntityIcon } from "@/features/entities/entity-icon";
import { lookFor } from "@/features/entities/type-look";
import type { EntitySummary, EntityTypeDef } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { moveItem, reassignTargets } from "./draft";
import { useDeleteEntityType, useReorderEntityTypes } from "./queries";

interface Props {
  types: EntityTypeDef[];
  entities: EntitySummary[];
  /** Key of the type open in the editor (null while creating a new one). */
  selectedKey: string | null;
  onSelect: (key: string) => void;
  onNew: () => void;
  /** Called after a type was deleted, with its key. */
  onDeleted: (key: string) => void;
}

/** Built-in types (editable fields only) and custom types (reorder, edit, delete). */
export function TypesList({ types, entities, selectedKey, onSelect, onNew, onDeleted }: Props) {
  const reorder = useReorderEntityTypes();
  const [deleting, setDeleting] = useState<EntityTypeDef | null>(null);

  const builtin = types.filter((t) => t.builtin);
  const custom = types.filter((t) => !t.builtin);

  function move(index: number, to: number) {
    const keys = moveItem(
      custom.map((t) => t.key),
      index,
      to,
    );
    reorder.mutate(keys);
  }

  function row(t: EntityTypeDef, actions: React.ReactNode) {
    return (
      <li
        key={t.key}
        className={cn(
          "flex flex-col gap-2 rounded-lg border p-3",
          t.key === selectedKey && "border-ring bg-accent/40",
        )}
      >
        <div className="flex items-center gap-2">
          <EntityIcon
            icon={t.icon}
            className="size-4 shrink-0"
            style={{ color: lookFor(t).color }}
          />
          <span className="truncate font-medium">{t.label}</span>
          <span className="truncate text-xs text-muted-foreground">{t.plural}</span>
          <span
            aria-hidden
            className="ml-auto size-3 shrink-0 rounded-full"
            style={{ backgroundColor: lookFor(t).color }}
          />
          {t.builtin && <Badge variant="secondary">built-in</Badge>}
        </div>
        <div className="flex flex-wrap gap-1">{actions}</div>
      </li>
    );
  }

  return (
    <section aria-label="Entity types" className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold">Your types</h2>
        <Button variant="outline" size="sm" onClick={onNew}>
          <Plus /> New type
        </Button>
      </div>
      {reorder.error && (
        <p role="alert" className="text-sm text-destructive">
          {reorder.error.message}
        </p>
      )}

      <h3 className="text-xs font-medium tracking-wide text-muted-foreground uppercase">
        Custom types
      </h3>
      {custom.length === 0 && (
        <p className="text-sm text-muted-foreground">
          None yet. Create one for deities, spells, ships, anything your world needs.
        </p>
      )}
      <ul className="flex flex-col gap-2">
        {custom.map((t, i) =>
          row(
            t,
            <>
              <Button
                variant="ghost"
                size="sm"
                aria-label={`Edit ${t.label}`}
                onClick={() => onSelect(t.key)}
              >
                <Pencil /> Edit
              </Button>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label={`Move ${t.label} up`}
                disabled={i === 0 || reorder.isPending}
                onClick={() => move(i, i - 1)}
              >
                <ArrowUp />
              </Button>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label={`Move ${t.label} down`}
                disabled={i === custom.length - 1 || reorder.isPending}
                onClick={() => move(i, i + 1)}
              >
                <ArrowDown />
              </Button>
              <Button
                variant="ghost"
                size="sm"
                aria-label={`Delete ${t.label}`}
                onClick={() => setDeleting(t)}
              >
                <Trash2 /> Delete
              </Button>
            </>,
          ),
        )}
      </ul>

      <h3 className="mt-2 text-xs font-medium tracking-wide text-muted-foreground uppercase">
        Built-in types
      </h3>
      <ul className="flex flex-col gap-2">
        {builtin.map((t) =>
          row(
            t,
            <Button
              variant="ghost"
              size="sm"
              aria-label={`Edit ${t.label}`}
              onClick={() => onSelect(t.key)}
            >
              <Pencil /> Extra fields
            </Button>,
          ),
        )}
      </ul>

      <DeleteTypeDialog
        key={deleting?.key ?? "none"}
        type={deleting}
        types={types}
        count={deleting ? entities.filter((e) => e.type === deleting.key).length : 0}
        onClose={() => setDeleting(null)}
        onDeleted={(key) => {
          setDeleting(null);
          onDeleted(key);
        }}
      />
    </section>
  );
}

function DeleteTypeDialog({
  type,
  types,
  count,
  onClose,
  onDeleted,
}: {
  type: EntityTypeDef | null;
  types: EntityTypeDef[];
  count: number;
  onClose: () => void;
  onDeleted: (key: string) => void;
}) {
  const remove = useDeleteEntityType();
  const targets = type ? reassignTargets(types, type.key) : [];
  const [target, setTarget] = useState<string>("");
  const needsTarget = count > 0;

  async function confirm() {
    if (!type) return;
    try {
      await remove.mutateAsync({ key: type.key, reassignTo: needsTarget ? target : null });
      onDeleted(type.key);
    } catch {
      // The error is shown inline below (mutation.error).
    }
  }

  return (
    <Dialog open={type !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Delete type “{type?.label}”?</DialogTitle>
          <DialogDescription>
            {needsTarget
              ? `${count} ${count === 1 ? "entity uses" : "entities use"} this type. Choose where they should go; the type itself is then removed.`
              : "No entities use this type. This cannot be undone."}
          </DialogDescription>
        </DialogHeader>
        {needsTarget && (
          <div className="flex flex-col gap-1.5">
            <label htmlFor="reassign-type" className="text-sm font-medium">
              Move {count} {count === 1 ? "entity" : "entities"} to type:
            </label>
            <NativeSelect
              id="reassign-type"
              value={target}
              onChange={(e) => setTarget(e.target.value)}
            >
              <option value="">Choose a type…</option>
              {targets.map((t) => (
                <option key={t.key} value={t.key}>
                  {t.label}
                </option>
              ))}
            </NativeSelect>
          </div>
        )}
        {remove.error && (
          <p role="alert" className="text-sm text-destructive">
            {remove.error.message}
          </p>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="destructive"
            disabled={remove.isPending || (needsTarget && target === "")}
            onClick={() => void confirm()}
          >
            {needsTarget ? "Move and delete" : "Delete"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
