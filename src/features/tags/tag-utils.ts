import type { Tag } from "@/lib/bindings";

export const MAX_TAG_LENGTH = 40;
export const MAX_TAGS_PER_ENTITY = 50;

/** Trims a typed tag and strips leading `#` characters (the backend refuses them). */
export function normalizeTagName(raw: string): string {
  return raw.trim().replace(/^#+/, "").trim();
}

// Letters, digits, `-`, `_` and spaces inside a segment; `/` separates segments.
const SEGMENT = /^[\p{L}\p{N}_\- ]+$/u;

/**
 * Client-side mirror of the backend rules, so the user gets feedback while
 * typing. The backend stays the authority and its message is shown as well.
 * Returns an error message, or `null` when the name is fine.
 */
export function validateTagName(name: string): string | null {
  if (name.length === 0) return "A tag needs a name.";
  if (name.length > MAX_TAG_LENGTH) return `Tags can have at most ${MAX_TAG_LENGTH} characters.`;
  for (const segment of name.split("/")) {
    if (segment.trim() === "") return "Nested tags cannot have empty parts (a//b, /a or a/).";
    if (!SEGMENT.test(segment)) {
      return "Use letters, digits, - _ and spaces. Use / for nested tags.";
    }
  }
  return null;
}

/** Case-insensitive comparison key for tag names. */
export function tagKey(name: string): string {
  return name.toLowerCase();
}

/** True when `tag` is `selected` itself or nested below it (`places` matches `places/cities`). */
export function tagMatches(tag: string, selected: string): boolean {
  const t = tagKey(tag);
  const s = tagKey(selected);
  return t === s || t.startsWith(`${s}/`);
}

/**
 * Does an entity with `entityTags` pass the filter? `matchAll` needs every
 * selected tag to match (nested-aware), otherwise one is enough.
 */
export function matchesTagFilter(
  entityTags: readonly string[],
  selected: readonly string[],
  matchAll: boolean,
): boolean {
  if (selected.length === 0) return true;
  const hit = (sel: string) => entityTags.some((t) => tagMatches(t, sel));
  return matchAll ? selected.every(hit) : selected.some(hit);
}

/** Lookup of colours by lower-case tag name. */
export function tagColorMap(tags: readonly Tag[]): Map<string, string> {
  const map = new Map<string, string>();
  for (const tag of tags) if (tag.color) map.set(tagKey(tag.name), tag.color);
  return map;
}

/** Tag names containing `typed` (case-insensitive), excluding `taken`, best matches first. */
export function suggestTags(
  all: readonly string[],
  typed: string,
  taken: readonly string[],
  limit = 6,
): string[] {
  const needle = tagKey(normalizeTagName(typed));
  const takenKeys = new Set(taken.map(tagKey));
  const pool = all.filter((n) => !takenKeys.has(tagKey(n)));
  const starts = pool.filter((n) => tagKey(n).startsWith(needle));
  const contains = pool.filter((n) => !tagKey(n).startsWith(needle) && tagKey(n).includes(needle));
  return [...starts, ...contains].slice(0, limit);
}
