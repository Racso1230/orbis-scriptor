import { matchesTagFilter } from "@/features/tags/tag-utils";
import type { EntitySummary, EntityTypeDef } from "@/lib/bindings";

export type SectionId = "favorites" | "recent";

export type SidebarRow =
  | { kind: "header"; type: EntityTypeDef; count: number; collapsed: boolean }
  | { kind: "section"; id: SectionId; label: string; count: number; collapsed: boolean }
  | { kind: "entity"; entity: EntitySummary; section?: SectionId };

/** Tags the sidebar is filtered by; `matchAll` needs every tag, otherwise one is enough. */
export interface TagFilter {
  tags: string[];
  matchAll: boolean;
}

/**
 * Flattens types + entities into the rows the virtualised list renders.
 * Pure so it can be unit tested without React.
 */
export function buildSidebarRows(
  types: EntityTypeDef[],
  entities: EntitySummary[],
  filter: string,
  collapsed: ReadonlySet<string>,
  tagFilter?: TagFilter,
): SidebarRow[] {
  const needle = filter.trim().toLowerCase();
  const tagsActive = (tagFilter?.tags.length ?? 0) > 0;
  const filtering = needle !== "" || tagsActive;
  const rows: SidebarRow[] = [];

  for (const type of types) {
    const matching = entities.filter(
      (e) =>
        e.type === type.key &&
        (needle === "" || e.name.toLowerCase().includes(needle)) &&
        (!tagFilter || matchesTagFilter(e.tags, tagFilter.tags, tagFilter.matchAll)),
    );
    // While filtering, hide empty groups and ignore collapsed state so results show.
    if (filtering && matching.length === 0) continue;
    const isCollapsed = !filtering && collapsed.has(type.key);

    rows.push({ kind: "header", type, count: matching.length, collapsed: isCollapsed });
    if (!isCollapsed) {
      for (const entity of matching) rows.push({ kind: "entity", entity });
    }
  }
  return rows;
}

/**
 * The "Favourites" and "Recently edited" sections shown above the type groups.
 * Empty sections are left out, and so is everything while a filter is active.
 */
export function buildTopSections(
  favorites: EntitySummary[],
  recent: EntitySummary[],
  collapsed: ReadonlySet<SectionId>,
  filtering: boolean,
): SidebarRow[] {
  if (filtering) return [];
  const rows: SidebarRow[] = [];
  const add = (id: SectionId, label: string, list: EntitySummary[]) => {
    if (list.length === 0) return;
    const isCollapsed = collapsed.has(id);
    rows.push({ kind: "section", id, label, count: list.length, collapsed: isCollapsed });
    if (!isCollapsed) for (const entity of list) rows.push({ kind: "entity", entity, section: id });
  };
  add("favorites", "Favourites", favorites);
  add("recent", "Recently edited", recent);
  return rows;
}
