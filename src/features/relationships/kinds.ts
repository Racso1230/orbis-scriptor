import type { RelationshipCategory, RelationshipKindDef } from "@/lib/bindings";

/** Display order and names for the category groups. */
export const CATEGORY_ORDER: RelationshipCategory[] = [
  "family",
  "social",
  "political",
  "place",
  "ownership",
  "other",
];

export const CATEGORY_LABELS: Record<RelationshipCategory, string> = {
  family: "Family",
  social: "Social",
  political: "Political",
  place: "Places",
  ownership: "Ownership",
  other: "Other",
};

/** Stroke colours per category (plain hex: used by SVG edges and legends). */
export const CATEGORY_COLORS: Record<RelationshipCategory, string> = {
  family: "#e11d48",
  social: "#2563eb",
  political: "#7c3aed",
  place: "#16a34a",
  ownership: "#d97706",
  other: "#64748b",
};

/**
 * One way of creating a relationship from the current entity's page.
 * `inverse` means the current entity is the *target* of the stored link, so the
 * option is worded with the inverse label ("child of") and the other entity
 * becomes the source.
 */
export interface KindOption {
  def: RelationshipKindDef;
  inverse: boolean;
  /** How the current entity relates to the other one: "parent of" / "child of". */
  label: string;
  /** Entity types the *other* entity may have; empty means any. */
  otherTypes: string[];
}

const allows = (types: string[], type: string) => types.length === 0 || types.includes(type);

/**
 * Kinds that can be created with an entity of `entityType` on this page.
 * Forward when the type may be the source, inverse when it may be the target.
 * Symmetric kinds are offered once unless only the target side fits.
 */
export function offeredKinds(kinds: RelationshipKindDef[], entityType: string): KindOption[] {
  const out: KindOption[] = [];
  for (const def of kinds) {
    const asSource = allows(def.sourceTypes, entityType);
    const asTarget = allows(def.targetTypes, entityType);
    if (asSource) {
      out.push({ def, inverse: false, label: def.forwardLabel, otherTypes: def.targetTypes });
    }
    if (asTarget && (!def.symmetric || !asSource)) {
      out.push({ def, inverse: true, label: def.inverseLabel, otherTypes: def.sourceTypes });
    }
  }
  return out;
}

/** Stable id for a `KindOption` (used as the select value). */
export function optionId(option: KindOption): string {
  return `${option.def.key}:${option.inverse ? "inverse" : "forward"}`;
}

/** Groups items by category in display order, dropping empty groups. */
export function groupByCategory<T extends { category: RelationshipCategory }>(
  items: T[],
): { category: RelationshipCategory; items: T[] }[] {
  return CATEGORY_ORDER.map((category) => ({
    category,
    items: items.filter((i) => i.category === category),
  })).filter((g) => g.items.length > 0);
}

/** Source and target ids for a new link, given which side the current entity is. */
export function endpoints(
  option: Pick<KindOption, "inverse">,
  currentId: string,
  otherId: string,
): { sourceId: string; targetId: string } {
  return option.inverse
    ? { sourceId: otherId, targetId: currentId }
    : { sourceId: currentId, targetId: otherId };
}

/**
 * Kinds an existing relationship could be changed to, keeping the same
 * direction: both endpoint types must be allowed by the kind.
 */
export function kindsForPair(
  kinds: RelationshipKindDef[],
  sourceType: string,
  targetType: string,
): RelationshipKindDef[] {
  return kinds.filter(
    (k) => allows(k.sourceTypes, sourceType) && allows(k.targetTypes, targetType),
  );
}
