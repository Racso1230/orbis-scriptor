import { ArrowDown, ArrowUp, Lock, Plus, Trash2, X } from "lucide-react";
import { NativeSelect } from "@/components/native-select";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { EntityTypeDef, FieldDef } from "@/lib/bindings";
import {
  KIND_LABELS,
  KIND_ORDER,
  isSelectKind,
  kindNameOf,
  optionsLost,
  type FieldDraft,
  type KindName,
} from "./draft";

/** A built-in field of a built-in type: shown, never editable. */
export function LockedField({ field }: { field: FieldDef }) {
  return (
    <li className="flex items-center gap-2 rounded-md border bg-muted/30 px-3 py-2 text-sm">
      <Lock className="size-3.5 text-muted-foreground" aria-hidden />
      <span className="font-medium">{field.label}</span>
      <span className="text-muted-foreground">{KIND_LABELS[kindNameOf(field.kind)]}</span>
      <span className="ml-auto text-xs text-muted-foreground">built-in</span>
    </li>
  );
}

interface Props {
  field: FieldDraft;
  index: number;
  count: number;
  /** Every type, for the "allowed types" choices of reference fields. */
  types: EntityTypeDef[];
  onChange: (field: FieldDraft) => void;
  onMove: (from: number, to: number) => void;
  onRemove: () => void;
}

/** Editor for one user-defined field: label, kind and the kind's own settings. */
export function FieldEditor({ field, index, count, types, onChange, onMove, onRemove }: Props) {
  const name = field.label.trim() || `field ${index + 1}`;

  function setKind(kind: KindName) {
    onChange({ ...field, kind });
  }
  function setOption(i: number, value: string) {
    onChange({ ...field, options: field.options.map((o, j) => (j === i ? value : o)) });
  }
  function toggleAllowed(key: string, on: boolean) {
    onChange({
      ...field,
      allowedTypes: on ? [...field.allowedTypes, key] : field.allowedTypes.filter((k) => k !== key),
    });
  }

  return (
    <li className="flex flex-col gap-3 rounded-md border p-3" aria-label={`Field ${name}`}>
      <div className="flex flex-wrap items-center gap-2">
        <Input
          aria-label={`Label of field ${index + 1}`}
          placeholder="Field label"
          value={field.label}
          maxLength={40}
          onChange={(e) => onChange({ ...field, label: e.target.value })}
          className="h-8 min-w-40 flex-1"
        />
        <NativeSelect
          aria-label={`Kind of ${name}`}
          value={field.kind}
          onChange={(e) => setKind(e.target.value as KindName)}
        >
          {KIND_ORDER.map((k) => (
            <option key={k} value={k}>
              {KIND_LABELS[k]}
            </option>
          ))}
        </NativeSelect>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label={`Move ${name} up`}
          disabled={index === 0}
          onClick={() => onMove(index, index - 1)}
        >
          <ArrowUp />
        </Button>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label={`Move ${name} down`}
          disabled={index === count - 1}
          onClick={() => onMove(index, index + 1)}
        >
          <ArrowDown />
        </Button>
        <Button variant="ghost" size="icon-xs" aria-label={`Remove ${name}`} onClick={onRemove}>
          <Trash2 />
        </Button>
      </div>

      {isSelectKind(field.kind) && (
        <div className="flex flex-col gap-2 pl-1">
          <p className="text-xs font-medium text-muted-foreground">Options</p>
          <ul className="flex flex-col gap-1.5">
            {field.options.map((option, i) => (
              <li key={i} className="flex items-center gap-2">
                <Input
                  aria-label={`Option ${i + 1} of ${name}`}
                  value={option}
                  maxLength={80}
                  onChange={(e) => setOption(i, e.target.value)}
                  className="h-8"
                />
                <Button
                  variant="ghost"
                  size="icon-xs"
                  aria-label={`Remove option ${option || i + 1} of ${name}`}
                  onClick={() =>
                    onChange({ ...field, options: field.options.filter((_, j) => j !== i) })
                  }
                >
                  <X />
                </Button>
              </li>
            ))}
          </ul>
          <div>
            <Button
              variant="outline"
              size="sm"
              onClick={() => onChange({ ...field, options: [...field.options, ""] })}
            >
              <Plus /> Add option
            </Button>
          </div>
          {field.originalKind === field.kind && optionsLost(field) && (
            <p className="text-xs text-amber-600 dark:text-amber-400">
              Renaming or removing an option leaves entities that used it unmatched: their old value
              is kept but shown empty.
            </p>
          )}
        </div>
      )}

      {field.kind === "entityRef" && (
        <div className="flex flex-col gap-2 pl-1">
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={field.multiple}
              onChange={(e) => onChange({ ...field, multiple: e.target.checked })}
            />
            Allow several references
          </label>
          <fieldset className="flex flex-col gap-1">
            <legend className="text-xs font-medium text-muted-foreground">
              Allowed types (none ticked = any entity)
            </legend>
            <div className="flex flex-wrap gap-x-4 gap-y-1">
              {types.map((t) => (
                <label key={t.key} className="flex items-center gap-1.5 text-sm">
                  <input
                    type="checkbox"
                    checked={field.allowedTypes.includes(t.key)}
                    onChange={(e) => toggleAllowed(t.key, e.target.checked)}
                  />
                  {t.plural}
                </label>
              ))}
            </div>
          </fieldset>
        </div>
      )}

      {field.originalKind !== null && field.originalKind !== field.kind && (
        <p className="text-xs text-amber-600 dark:text-amber-400">
          Changing the kind keeps stored values, but values that do not fit are shown empty.
        </p>
      )}
    </li>
  );
}
