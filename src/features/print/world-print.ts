import type { EntitySummary, EntityTypeDef } from "@/lib/bindings";

/** Printing more entities than this in one go is slow and the PDF unwieldy. */
export const WORLD_PRINT_LIMIT = 500;

/** Entities grouped in type order (as in the sidebar), each group sorted by name. */
export function sortForPrint(entities: EntitySummary[], types: EntityTypeDef[]): EntitySummary[] {
  const order = new Map(types.map((t, i) => [t.key, i] as const));
  return [...entities].sort(
    (a, b) =>
      (order.get(a.type) ?? Number.MAX_SAFE_INTEGER) -
        (order.get(b.type) ?? Number.MAX_SAFE_INTEGER) || a.name.localeCompare(b.name),
  );
}

/** The first `limit` entities, and how many there were in total. */
export function capEntities(
  entities: EntitySummary[],
  limit = WORLD_PRINT_LIMIT,
): { shown: EntitySummary[]; total: number } {
  return { shown: entities.slice(0, limit), total: entities.length };
}

/** Table of contents: one group per type that has entities, in type order. */
export function tocGroups(
  entities: EntitySummary[],
  types: EntityTypeDef[],
): { type: EntityTypeDef; entities: EntitySummary[] }[] {
  return types
    .map((type) => ({ type, entities: entities.filter((e) => e.type === type.key) }))
    .filter((g) => g.entities.length > 0);
}
