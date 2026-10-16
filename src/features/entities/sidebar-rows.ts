import type { EntitySummary, EntityTypeDef } from "@/lib/bindings";

export type SidebarRow =
  | { kind: "header"; type: EntityTypeDef; count: number; collapsed: boolean }
  | { kind: "entity"; entity: EntitySummary };

/**
 * Flattens types + entities into the rows the virtualised list renders.
 * Pure so it can be unit tested without React.
 */
export function buildSidebarRows(
  types: EntityTypeDef[],
  entities: EntitySummary[],
  filter: string,
  collapsed: ReadonlySet<string>,
): SidebarRow[] {
  const needle = filter.trim().toLowerCase();
  const rows: SidebarRow[] = [];

  for (const type of types) {
    const matching = entities.filter(
      (e) => e.type === type.key && (needle === "" || e.name.toLowerCase().includes(needle)),
    );
    // While filtering, hide empty groups and ignore collapsed state so results show.
    if (needle !== "" && matching.length === 0) continue;
    const isCollapsed = needle === "" && collapsed.has(type.key);

    rows.push({ kind: "header", type, count: matching.length, collapsed: isCollapsed });
    if (!isCollapsed) {
      for (const entity of matching) rows.push({ kind: "entity", entity });
    }
  }
  return rows;
}
