import { useEffect, useMemo, useRef, useState } from "react";
import { Plus, Save, TriangleAlert } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { EntityIcon } from "@/features/entities/entity-icon";
import type { EntityTypeDef } from "@/lib/bindings";
import {
  MAX_FIELDS,
  describeRisks,
  draftFromType,
  draftSignature,
  isHexColor,
  moveItem,
  newFieldDraft,
  newTypeDraft,
  toFieldInputs,
  withLabel,
  withPlural,
  type FieldDraft,
  type TypeDraft,
} from "./draft";
import { FieldEditor, LockedField } from "./field-editor";
import { ColorPicker, IconPicker } from "./pickers";
import {
  useCreateEntityType,
  useSetTypeTemplateFields,
  useTypeIcons,
  useUpdateEntityType,
} from "./queries";

interface Props {
  /** The type to edit, or null to create a new custom type. */
  type: EntityTypeDef | null;
  /** Every type (for the allowed-types choices). */
  types: EntityTypeDef[];
  onSaved: (type: EntityTypeDef) => void;
  /** Reports whether there are unsaved changes (for the leave-page guard). */
  onDirtyChange: (dirty: boolean) => void;
}

/** Form for one entity type: look, plural and fields. Built-in types only get extra fields. */
export function TypeEditor({ type, types, onSaved, onDirtyChange }: Props) {
  const create = useCreateEntityType();
  const update = useUpdateEntityType();
  const setTemplate = useSetTypeTemplateFields();
  const icons = useTypeIcons();

  const [initial, setInitial] = useState<TypeDraft>(() =>
    type ? draftFromType(type) : newTypeDraft(),
  );
  const [draft, setDraft] = useState<TypeDraft>(initial);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  const dirty = draftSignature(draft) !== draftSignature(initial);
  const onDirtyRef = useRef(onDirtyChange);
  useEffect(() => {
    onDirtyRef.current = onDirtyChange;
  });
  useEffect(() => {
    onDirtyRef.current(dirty);
  }, [dirty]);
  // Leaving the editor clears the flag.
  useEffect(() => () => onDirtyRef.current(false), []);

  const risks = useMemo(() => describeRisks(initial, draft), [initial, draft]);
  const builtinFields = type?.builtin ? type.fields.filter((f) => !f.custom) : [];

  function edit(next: TypeDraft) {
    setDraft(next);
    setError(null);
    setSaved(false);
  }
  function setField(uid: string, field: FieldDraft) {
    edit({ ...draft, fields: draft.fields.map((f) => (f.uid === uid ? field : f)) });
  }

  const labelOk = draft.builtin || (draft.label.trim() !== "" && draft.plural.trim() !== "");
  const fieldsOk = draft.fields.every((f) => f.label.trim() !== "");
  const colorOk = draft.builtin || isHexColor(draft.color);
  const busy = create.isPending || update.isPending || setTemplate.isPending;

  async function save() {
    const fields = toFieldInputs(draft.fields);
    setError(null);
    try {
      let result: EntityTypeDef;
      if (draft.builtin && draft.key) {
        result = await setTemplate.mutateAsync({ typeKey: draft.key, fields });
      } else {
        const values = {
          label: draft.label.trim(),
          plural: draft.plural.trim(),
          icon: draft.icon,
          color: draft.color.toLowerCase(),
          fields,
        };
        result = draft.key
          ? await update.mutateAsync({ key: draft.key, ...values })
          : await create.mutateAsync(values);
      }
      const fresh = draftFromType(result);
      setInitial(fresh);
      setDraft(fresh);
      setSaved(true);
      onSaved(result);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  return (
    <div className="flex flex-col gap-5">
      {draft.builtin ? (
        <p className="rounded-md border bg-muted/30 px-3 py-2 text-sm text-muted-foreground">
          Built-in types keep their name, icon and colour. You can add extra fields to them.
        </p>
      ) : (
        <>
          <div className="grid gap-3 sm:grid-cols-2">
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="type-label">Name (singular)</Label>
              <Input
                id="type-label"
                value={draft.label}
                maxLength={40}
                placeholder="Deity"
                onChange={(e) => edit(withLabel(draft, e.target.value))}
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="type-plural">Plural</Label>
              <Input
                id="type-plural"
                value={draft.plural}
                maxLength={40}
                placeholder="Deities"
                onChange={(e) => edit(withPlural(draft, e.target.value))}
              />
            </div>
          </div>
          <div className="flex flex-col gap-1.5">
            <Label>Icon</Label>
            <IconPicker
              icons={icons.data ?? []}
              value={draft.icon}
              color={isHexColor(draft.color) ? draft.color : "#64748b"}
              onChange={(icon) => edit({ ...draft, icon })}
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <Label>Colour</Label>
            <ColorPicker value={draft.color} onChange={(color) => edit({ ...draft, color })} />
          </div>
          <p className="flex items-center gap-2 text-sm text-muted-foreground">
            Preview:
            <EntityIcon
              icon={draft.icon}
              className="size-4"
              style={{ color: isHexColor(draft.color) ? draft.color : undefined }}
            />
            <span className="font-medium text-foreground">{draft.label || "New type"}</span>
          </p>
        </>
      )}

      <section aria-label="Fields" className="flex flex-col gap-3">
        <h3 className="text-sm font-semibold">
          {draft.builtin ? "Extra fields" : "Fields"} ({draft.fields.length}/{MAX_FIELDS})
        </h3>
        {builtinFields.length > 0 && (
          <ul className="flex flex-col gap-2" aria-label="Built-in fields">
            {builtinFields.map((f) => (
              <LockedField key={f.key} field={f} />
            ))}
          </ul>
        )}
        {draft.fields.length > 0 && (
          <ul className="flex flex-col gap-2">
            {draft.fields.map((field, i) => (
              <FieldEditor
                key={field.uid}
                field={field}
                index={i}
                count={draft.fields.length}
                types={types}
                onChange={(next) => setField(field.uid, next)}
                onMove={(from, to) => edit({ ...draft, fields: moveItem(draft.fields, from, to) })}
                onRemove={() => edit({ ...draft, fields: draft.fields.filter((f) => f !== field) })}
              />
            ))}
          </ul>
        )}
        {draft.fields.length === 0 && (
          <p className="text-sm text-muted-foreground">No fields yet.</p>
        )}
        <div>
          <Button
            variant="outline"
            size="sm"
            disabled={draft.fields.length >= MAX_FIELDS}
            onClick={() => edit({ ...draft, fields: [...draft.fields, newFieldDraft()] })}
          >
            <Plus /> Add field
          </Button>
        </div>
      </section>

      {risks.length > 0 && (
        <ul
          aria-label="Things to know before saving"
          className="flex flex-col gap-1 rounded-md border border-amber-500/40 bg-amber-500/10 p-3 text-xs"
        >
          {risks.map((risk, i) => (
            <li key={i} className="flex gap-2">
              <TriangleAlert className="mt-0.5 size-3.5 shrink-0 text-amber-600" aria-hidden />
              {risk}
            </li>
          ))}
        </ul>
      )}

      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}

      <div className="flex items-center gap-3">
        <Button
          onClick={() => void save()}
          disabled={busy || !dirty || !labelOk || !fieldsOk || !colorOk}
        >
          <Save /> {type ? "Save changes" : "Create type"}
        </Button>
        {saved && !dirty && (
          <span role="status" className="text-sm text-muted-foreground">
            Saved.
          </span>
        )}
        {!fieldsOk && (
          <span className="text-xs text-muted-foreground">Every field needs a label.</span>
        )}
      </div>
    </div>
  );
}
