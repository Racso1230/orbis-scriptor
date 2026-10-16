import type { EntitySummary, EntityTypeDef } from "@/lib/bindings";

/** One row of the `[[` picker: an existing entity or a "create new" option. */
export type MentionItem =
  | { kind: "entity"; id: string; title: string; description: string }
  | { kind: "create"; type: string; name: string; title: string; description: string };

const MAX_MATCHES = 8;

/**
 * Entities whose name contains the query (best matches first), followed by
 * one "create" option per type when the query is not empty. Pure for tests.
 */
export function buildMentionItems(
  entities: EntitySummary[],
  types: EntityTypeDef[],
  query: string,
): MentionItem[] {
  const q = query.trim().toLowerCase();
  const label = (typeKey: string) => types.find((t) => t.key === typeKey)?.label ?? typeKey;

  const matches = entities
    .filter((e) => q === "" || e.name.toLowerCase().includes(q))
    .sort((a, b) => {
      // Names that start with the query rank above names that merely contain it.
      const aStarts = a.name.toLowerCase().startsWith(q) ? 0 : 1;
      const bStarts = b.name.toLowerCase().startsWith(q) ? 0 : 1;
      return aStarts - bStarts || a.name.localeCompare(b.name);
    })
    .slice(0, MAX_MATCHES)
    .map<MentionItem>((e) => ({
      kind: "entity",
      id: e.id,
      title: e.name,
      description: label(e.type),
    }));

  const creates =
    q === ""
      ? []
      : types.map<MentionItem>((t) => ({
          kind: "create",
          type: t.key,
          name: query.trim(),
          title: `Create ${t.label.toLowerCase()} “${query.trim()}”`,
          description: "New entity",
        }));

  return [...matches, ...creates];
}
