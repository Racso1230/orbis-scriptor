import { useId, useLayoutEffect, useRef, useState } from "react";
import { ExternalLink, Plus, X } from "lucide-react";
import { NativeSelect } from "@/components/native-select";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { useCalendars, useDisplayCalendar } from "@/features/calendars/queries";
import { EntityPickerDialog } from "@/features/maps/entity-picker-dialog";
import { formatDate } from "@/features/timeline/calendar";
import { DateEditor } from "@/features/timeline/date-editor";
import type { FieldDef } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { EntityIcon } from "./entity-icon";
import { coerceFieldValue, fieldErrorFor, isEntityRef, isWideKind } from "./field-values";
import { useCreateEntity, useEntities, useEntityTypes } from "./queries";
import { lookFor } from "./type-look";
import { useOpenEntity } from "./use-open-entity";

interface FieldInputProps {
  field: FieldDef;
  /** The raw stored value; the input coerces it itself. */
  value: unknown;
  /** `undefined` clears the field (the form then removes the key). */
  onChange: (value: unknown) => void;
  /** The entity being edited (a reference field cannot sensibly pick itself). */
  entityId?: string;
  /** A server validation message; shown when it starts with this field's label. */
  error?: string | null;
}

/** One labelled input for any field kind (see `coerceFieldValue` for the stored shapes). */
export function FieldInput({ field, value, onChange, entityId, error }: FieldInputProps) {
  const inputId = `field-${field.key}`;
  const problem = fieldErrorFor(field, error ?? null);
  const kind = field.kind;
  const coerced = coerceFieldValue(kind, value, field.options);

  let input: React.ReactNode;
  if (isEntityRef(kind)) {
    input = (
      <EntityRefInput
        id={inputId}
        label={field.label}
        allowedTypes={kind.entityRef.allowedTypes}
        multiple={kind.entityRef.multiple}
        value={coerced}
        entityId={entityId}
        onChange={onChange}
      />
    );
  } else if (kind === "select") {
    input = (
      <NativeSelect
        id={inputId}
        className="w-full"
        value={typeof coerced === "string" ? coerced : ""}
        aria-invalid={problem ? true : undefined}
        onChange={(e) => onChange(e.target.value === "" ? undefined : e.target.value)}
      >
        <option value="">—</option>
        {field.options.map((option) => (
          <option key={option} value={option}>
            {option}
          </option>
        ))}
      </NativeSelect>
    );
  } else if (kind === "multiSelect") {
    const selected = Array.isArray(coerced) ? coerced : [];
    input = (
      <div
        id={inputId}
        role="group"
        aria-label={field.label}
        className="flex flex-wrap gap-x-4 gap-y-1"
      >
        {field.options.map((option) => (
          <label key={option} className="flex items-center gap-1.5 text-sm">
            <input
              type="checkbox"
              checked={selected.includes(option)}
              onChange={(e) => {
                const next = e.target.checked
                  ? [...selected, option]
                  : selected.filter((s) => s !== option);
                // Keep the options' order so the stored list is stable.
                onChange(field.options.filter((o) => next.includes(o)));
              }}
            />
            {option}
          </label>
        ))}
      </div>
    );
  } else if (kind === "number") {
    input = (
      <Input
        id={inputId}
        type="number"
        value={typeof coerced === "number" ? coerced : ""}
        aria-invalid={problem ? true : undefined}
        onChange={(e) => onChange(e.target.value === "" ? undefined : Number(e.target.value))}
      />
    );
  } else if (kind === "boolean") {
    input = (
      <label className="flex items-center gap-2 text-sm">
        <input
          id={inputId}
          type="checkbox"
          checked={coerced === true}
          onChange={(e) => onChange(e.target.checked ? true : undefined)}
        />
        Yes
      </label>
    );
  } else if (kind === "longText") {
    input = (
      <AutoGrowTextarea
        id={inputId}
        value={typeof coerced === "string" ? coerced : ""}
        invalid={problem !== null}
        onChange={onChange}
      />
    );
  } else if (kind === "date") {
    input = (
      <DateFieldInput
        label={field.label}
        value={typeof coerced === "number" ? coerced : undefined}
        onChange={onChange}
      />
    );
  } else if (kind === "url") {
    const url = typeof coerced === "string" ? coerced : "";
    input = (
      <div className="flex items-center gap-2">
        <Input
          id={inputId}
          type="url"
          inputMode="url"
          placeholder="https://…"
          value={url}
          aria-invalid={problem ? true : undefined}
          onChange={(e) => onChange(e.target.value)}
        />
        {/^(https?:\/\/|mailto:)/i.test(url) && (
          // A plain link: the app has no "open in system browser" plugin.
          <a
            href={url}
            target="_blank"
            rel="noreferrer noopener"
            aria-label={`Open ${field.label} link`}
            className="shrink-0 text-muted-foreground hover:text-foreground"
          >
            <ExternalLink className="size-4" />
          </a>
        )}
      </div>
    );
  } else {
    input = (
      <Input
        id={inputId}
        maxLength={2000}
        value={typeof coerced === "string" ? coerced : ""}
        aria-invalid={problem ? true : undefined}
        onChange={(e) => onChange(e.target.value)}
      />
    );
  }

  return (
    <div className={cn("flex flex-col gap-2", isWideKind(field.kind) && "col-span-2")}>
      {kind !== "date" && <Label htmlFor={inputId}>{field.label}</Label>}
      {input}
      {problem && (
        <p role="alert" className="text-xs text-destructive">
          {problem}
        </p>
      )}
    </div>
  );
}

function AutoGrowTextarea({
  id,
  value,
  invalid,
  onChange,
}: {
  id: string;
  value: string;
  invalid: boolean;
  onChange: (value: unknown) => void;
}) {
  const ref = useRef<HTMLTextAreaElement>(null);
  // Resize to fit the content after every change.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight}px`;
  }, [value]);
  return (
    <Textarea
      ref={ref}
      id={id}
      rows={3}
      maxLength={50000}
      className="resize-none overflow-hidden"
      aria-invalid={invalid ? true : undefined}
      value={value}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

/** A date field: "Set date" until a value exists, then the date editor plus a clear button. */
function DateFieldInput({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number | undefined;
  onChange: (value: unknown) => void;
}) {
  const calendars = useCalendars();
  const display = useDisplayCalendar();
  const [calendarId, setCalendarId] = useState<string | null>(null);
  const list = calendars.data ?? [];
  const activeId = calendarId ?? display.data?.id ?? list[0]?.id ?? "";
  const active = list.find((c) => c.id === activeId) ?? list[0];

  if (!active) return <p className="text-sm text-muted-foreground">Loading calendars…</p>;

  if (value === undefined) {
    return (
      <div className="flex items-center gap-3">
        <span className="text-sm font-medium">{label}</span>
        <Button variant="outline" size="sm" onClick={() => onChange(0)}>
          <Plus /> Set date
        </Button>
      </div>
    );
  }

  return (
    <div className="flex items-start gap-2">
      <div className="min-w-0 flex-1">
        <DateEditor
          calendars={list}
          calendarId={active.id}
          day={value}
          label={`${label} (${formatDate(active.definition, value, "full")})`}
          onChange={onChange}
          onCalendarChange={setCalendarId}
        />
      </div>
      <Button
        variant="ghost"
        size="icon-xs"
        aria-label={`Clear ${label}`}
        onClick={() => onChange(undefined)}
      >
        <X />
      </Button>
    </div>
  );
}

/** Chips for the referenced entities, with remove buttons and an "Add…" picker. */
function EntityRefInput({
  id,
  label,
  allowedTypes,
  multiple,
  value,
  entityId,
  onChange,
}: {
  id: string;
  label: string;
  allowedTypes: string[];
  multiple: boolean;
  value: unknown;
  entityId?: string;
  onChange: (value: unknown) => void;
}) {
  const entities = useEntities();
  const types = useEntityTypes();
  const createEntity = useCreateEntity();
  const open = useOpenEntity();
  const [picking, setPicking] = useState(false);
  const groupId = useId();

  const ids: string[] = Array.isArray(value)
    ? (value as string[])
    : typeof value === "string"
      ? [value]
      : [];

  function add(newId: string) {
    setPicking(false);
    if (ids.includes(newId)) return;
    onChange(multiple ? [...ids, newId] : newId);
  }
  function remove(removeId: string) {
    const next = ids.filter((i) => i !== removeId);
    onChange(multiple ? next : undefined);
  }

  return (
    <div
      id={id}
      role="group"
      aria-labelledby={groupId}
      className="flex flex-wrap items-center gap-2"
    >
      <span id={groupId} className="sr-only">
        {label}
      </span>
      {ids.map((refId) => {
        const entity = entities.data?.find((e) => e.id === refId);
        const type = entity ? types.data?.find((t) => t.key === entity.type) : undefined;
        return (
          <span
            key={refId}
            className="inline-flex items-center gap-1 rounded-md border bg-muted/40 py-0.5 pr-0.5 pl-2 text-sm"
          >
            {entity ? (
              <button
                type="button"
                onClick={() => open(refId)}
                className="inline-flex items-center gap-1.5 hover:underline"
              >
                <EntityIcon
                  icon={type?.icon ?? ""}
                  className="size-3.5"
                  style={{ color: lookFor(type).color }}
                />
                {entity.name}
              </button>
            ) : (
              // Dangling ids are allowed (the entity was deleted): show, but keep removable.
              <span className="text-muted-foreground italic">missing entity</span>
            )}
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label={`Remove ${entity?.name ?? "missing entity"} from ${label}`}
              onClick={() => remove(refId)}
            >
              <X />
            </Button>
          </span>
        );
      })}
      {(multiple || ids.length === 0) && (
        <Button variant="outline" size="sm" onClick={() => setPicking(true)}>
          <Plus /> Add…
        </Button>
      )}
      <EntityPickerDialog
        open={picking}
        onOpenChange={setPicking}
        title={`Choose ${label}`}
        description="Pick an existing entity or create a new one"
        allowedTypes={allowedTypes}
        excludeId={entityId}
        onPickEntity={add}
        onCreateEntity={(type, name) => {
          void createEntity.mutateAsync({ type, name }).then((entity) => add(entity.id));
        }}
      />
    </div>
  );
}
