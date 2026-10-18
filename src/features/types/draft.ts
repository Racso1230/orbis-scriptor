import type { EntityTypeDef, FieldDef, FieldInput, FieldKind } from "@/lib/bindings";
import { isEntityRef } from "@/features/entities/field-values";

/** The kinds a user can pick in the editor (the entity reference variant is flattened). */
export type KindName =
  | "text"
  | "longText"
  | "number"
  | "boolean"
  | "select"
  | "multiSelect"
  | "date"
  | "url"
  | "entityRef";

export const KIND_LABELS: Record<KindName, string> = {
  text: "Text",
  longText: "Long text",
  number: "Number",
  boolean: "Yes / no",
  select: "Choice (one)",
  multiSelect: "Choice (several)",
  date: "Date",
  url: "Link (URL)",
  entityRef: "Entity reference",
};

export const KIND_ORDER = Object.keys(KIND_LABELS) as KindName[];

/** Colour offered by default for a new type (the backend uses the same one). */
export const DEFAULT_COLOR = "#0891b2";

export const COLOR_SWATCHES = [
  "#2563eb",
  "#0891b2",
  "#16a34a",
  "#ca8a04",
  "#d97706",
  "#dc2626",
  "#db2777",
  "#a855f7",
  "#7c3aed",
  "#64748b",
];

export const MAX_FIELDS = 40;

/** One field row in the editor. */
export interface FieldDraft {
  /** Local React key; never sent to the backend. */
  uid: string;
  /** The stored key; `null` for a field added in this session. */
  key: string | null;
  label: string;
  kind: KindName;
  options: string[];
  allowedTypes: string[];
  multiple: boolean;
  /** The kind when the editor opened, `null` for new fields (to warn on kind changes). */
  originalKind: KindName | null;
  /** The options when the editor opened (to warn on renamed or removed options). */
  originalOptions: string[];
}

export interface TypeDraft {
  /** `null` while creating a new type. */
  key: string | null;
  builtin: boolean;
  label: string;
  plural: string;
  /** False until the user edits the plural by hand: then it follows the label. */
  pluralTouched: boolean;
  icon: string;
  color: string;
  /** For built-in types these are only the extra (user-added) fields. */
  fields: FieldDraft[];
}

let counter = 0;
function newUid(): string {
  counter += 1;
  return `f${counter}-${Math.random().toString(36).slice(2, 8)}`;
}

export function kindNameOf(kind: FieldKind): KindName {
  return isEntityRef(kind) ? "entityRef" : kind;
}

export function fieldDraftFromDef(def: FieldDef): FieldDraft {
  const ref = isEntityRef(def.kind) ? def.kind.entityRef : null;
  const kind = kindNameOf(def.kind);
  return {
    uid: newUid(),
    key: def.key,
    label: def.label,
    kind,
    options: [...def.options],
    allowedTypes: ref ? [...ref.allowedTypes] : [],
    multiple: ref ? ref.multiple : false,
    originalKind: kind,
    originalOptions: [...def.options],
  };
}

export function newFieldDraft(): FieldDraft {
  return {
    uid: newUid(),
    key: null,
    label: "",
    kind: "text",
    options: [],
    allowedTypes: [],
    multiple: false,
    originalKind: null,
    originalOptions: [],
  };
}

export function newTypeDraft(): TypeDraft {
  return {
    key: null,
    builtin: false,
    label: "",
    plural: "",
    pluralTouched: false,
    icon: "sparkles",
    color: DEFAULT_COLOR,
    fields: [],
  };
}

/** Draft of an existing type. Built-in types only expose their user-added fields. */
export function draftFromType(type: EntityTypeDef): TypeDraft {
  return {
    key: type.key,
    builtin: type.builtin,
    label: type.label,
    plural: type.plural,
    pluralTouched: true,
    icon: type.icon,
    color: type.color,
    fields: type.fields.filter((f) => f.custom || !type.builtin).map(fieldDraftFromDef),
  };
}

/** The plural suggested for a label, until the user types their own. */
export function suggestPlural(label: string): string {
  const trimmed = label.trim();
  return trimmed === "" ? "" : `${trimmed}s`;
}

/** Changes the label; the plural follows along while it has not been edited by hand. */
export function withLabel(draft: TypeDraft, label: string): TypeDraft {
  return {
    ...draft,
    label,
    plural: draft.pluralTouched ? draft.plural : suggestPlural(label),
  };
}

export function withPlural(draft: TypeDraft, plural: string): TypeDraft {
  return { ...draft, plural, pluralTouched: true };
}

export function isSelectKind(kind: KindName): boolean {
  return kind === "select" || kind === "multiSelect";
}

/** The field as the backend expects it. Existing fields keep their `key`, new ones send `null`. */
export function toFieldInput(field: FieldDraft): FieldInput {
  const kind: FieldKind =
    field.kind === "entityRef"
      ? { entityRef: { allowedTypes: field.allowedTypes, multiple: field.multiple } }
      : field.kind;
  return {
    key: field.key,
    label: field.label.trim(),
    kind,
    options: isSelectKind(field.kind)
      ? field.options.map((o) => o.trim()).filter((o) => o !== "")
      : [],
  };
}

export function toFieldInputs(fields: FieldDraft[]): FieldInput[] {
  return fields.map(toFieldInput);
}

/** True for `#rrggbb`. */
export function isHexColor(value: string): boolean {
  return /^#[0-9a-fA-F]{6}$/.test(value);
}

/** Stable string that changes whenever the user changed something (ignores local ids). */
export function draftSignature(draft: TypeDraft): string {
  return JSON.stringify({
    key: draft.key,
    label: draft.label,
    plural: draft.plural,
    icon: draft.icon,
    color: draft.color.toLowerCase(),
    fields: draft.fields.map((f) => ({
      key: f.key,
      label: f.label,
      kind: f.kind,
      options: f.options,
      allowedTypes: f.allowedTypes,
      multiple: f.multiple,
    })),
  });
}

/** What the user is about to do to existing data, in plain words (shown before saving). */
export function describeRisks(initial: TypeDraft, draft: TypeDraft): string[] {
  const out: string[] = [];
  const keptKeys = new Set(draft.fields.map((f) => f.key).filter((k) => k !== null));
  for (const f of initial.fields) {
    if (f.key !== null && !keptKeys.has(f.key)) {
      out.push(
        `Removing “${f.label}” hides its values on entities. They stay stored and come back if a field with the same key is added again.`,
      );
    }
  }
  for (const f of draft.fields) {
    if (f.originalKind !== null && f.originalKind !== f.kind) {
      out.push(
        `Changing “${f.label}” from ${KIND_LABELS[f.originalKind].toLowerCase()} to ${KIND_LABELS[f.kind].toLowerCase()}: values that do not fit the new kind are kept but shown empty.`,
      );
    }
    if (isSelectKind(f.kind) && f.originalKind === f.kind && optionsLost(f)) {
      out.push(
        `Renaming or removing options of “${f.label}” leaves entities that used the old option unmatched (shown empty).`,
      );
    }
  }
  return out;
}

/** True when an option that existed when the editor opened is no longer in the list. */
export function optionsLost(field: FieldDraft): boolean {
  return field.originalOptions.some((o) => !field.options.includes(o));
}

/** Moves item `from` to `to` (both clamped); returns a new array. */
export function moveItem<T>(items: T[], from: number, to: number): T[] {
  if (from < 0 || from >= items.length || to < 0 || to >= items.length || from === to) {
    return items;
  }
  const next = [...items];
  const [item] = next.splice(from, 1);
  if (item === undefined) return items;
  next.splice(to, 0, item);
  return next;
}

/** Types an entity may be moved to when its own type is deleted (never itself, map or event). */
export function reassignTargets(types: EntityTypeDef[], deleting: string): EntityTypeDef[] {
  return types.filter((t) => t.key !== deleting && t.key !== "map" && t.key !== "event");
}
