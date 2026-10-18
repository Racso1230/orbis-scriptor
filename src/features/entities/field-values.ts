import type { FieldDef, FieldKind } from "@/lib/bindings";

/** What a field input works with after `coerceFieldValue`; `undefined` means "not set". */
export type FieldValue = string | number | boolean | string[] | undefined;

/** The `entityRef` variant of `FieldKind`, narrowed. */
export type EntityRefKind = Extract<FieldKind, { entityRef: unknown }>;

export function isEntityRef(kind: FieldKind): kind is EntityRefKind {
  return typeof kind === "object" && "entityRef" in kind;
}

/** Stable name of a kind, for labels and `switch`es ("entityRef" for the object variant). */
export function kindName(kind: FieldKind): string {
  return isEntityRef(kind) ? "entityRef" : kind;
}

function onlyStrings(value: unknown): string[] | undefined {
  if (!Array.isArray(value)) return undefined;
  const strings = value.filter((v): v is string => typeof v === "string");
  // Same shape check as the server: a mixed array is not a list of text values.
  return strings.length === value.length ? strings : undefined;
}

/**
 * Turns whatever is stored in `properties[key]` into a value the input for
 * `kind` can show. Stored values may no longer fit the field (an option was
 * removed, the kind changed, someone edited the file): those become `undefined`
 * ("empty") instead of crashing. The stored data itself is never touched here.
 *
 * `options` are the field's current options (only used for select kinds).
 */
export function coerceFieldValue(
  kind: FieldKind,
  value: unknown,
  options: string[] = [],
): FieldValue {
  if (value === null || value === undefined) return undefined;
  if (isEntityRef(kind)) {
    if (kind.entityRef.multiple) {
      const ids = onlyStrings(value);
      const unique = ids ? [...new Set(ids)] : [];
      return unique.length > 0 ? unique : undefined;
    }
    return typeof value === "string" && value !== "" ? value : undefined;
  }
  switch (kind) {
    case "text":
    case "longText":
    case "url":
      return typeof value === "string" && value !== "" ? value : undefined;
    case "number":
      return typeof value === "number" && Number.isFinite(value) ? value : undefined;
    case "boolean":
      return typeof value === "boolean" ? value : undefined;
    case "date":
      return typeof value === "number" && Number.isInteger(value) ? value : undefined;
    case "select":
      return typeof value === "string" && options.includes(value) ? value : undefined;
    case "multiSelect": {
      const items = onlyStrings(value);
      if (!items) return undefined;
      // Keep the options' order and drop duplicates and options that no longer exist.
      const kept = options.filter((o) => items.includes(o));
      return kept.length > 0 ? kept : undefined;
    }
  }
}

/** Kinds that need the full width of the form grid. */
export function isWideKind(kind: FieldKind): boolean {
  return kind === "longText" || kind === "multiSelect" || kind === "date" || isEntityRef(kind);
}

/** True when a value should be removed from `properties` instead of stored. */
export function isUnset(value: unknown): boolean {
  return (
    value === undefined ||
    value === null ||
    value === "" ||
    (Array.isArray(value) && value.length === 0)
  );
}

/**
 * Server validation messages look like "<field label>: <problem>". Returns the
 * problem when the message is about this field, otherwise null.
 */
export function fieldErrorFor(
  field: Pick<FieldDef, "label">,
  message: string | null,
): string | null {
  if (!message) return null;
  const prefix = `${field.label}: `;
  return message.startsWith(prefix) ? message.slice(prefix.length) : null;
}
