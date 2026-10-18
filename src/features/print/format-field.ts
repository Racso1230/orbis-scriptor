import { coerceFieldValue, isEntityRef } from "@/features/entities/field-values";
import { formatDate } from "@/features/timeline/calendar";
import type { CalendarDefinition, FieldDef } from "@/lib/bindings";

export interface FormatContext {
  /** The display calendar's definition (for date fields); may be missing while loading. */
  calendar: CalendarDefinition | undefined;
  /** Name of an entity by id, or undefined when it no longer exists. */
  nameOf: (entityId: string) => string | undefined;
}

/**
 * A field value as plain text for the print view, or `null` when the field is
 * empty (the row is then left out). Values that no longer fit the field show as empty.
 */
export function formatFieldValue(
  field: Pick<FieldDef, "kind" | "options">,
  value: unknown,
  ctx: FormatContext,
): string | null {
  const v = coerceFieldValue(field.kind, value, field.options);
  if (v === undefined) return null;
  if (isEntityRef(field.kind)) {
    const ids = Array.isArray(v) ? v : [String(v)];
    return ids.map((id) => ctx.nameOf(id) ?? "missing entity").join(", ");
  }
  if (field.kind === "date") {
    return typeof v === "number" && ctx.calendar ? formatDate(ctx.calendar, v, "full") : null;
  }
  if (typeof v === "boolean") return v ? "Yes" : "No";
  if (Array.isArray(v)) return v.join(", ");
  return String(v);
}
