/** The `tag:` filter the user is typing at the end of the search box. */
export interface TagToken {
  /** Where `tag:` starts in the query. */
  start: number;
  /** What was typed after `tag:` (without a quote). */
  partial: string;
}

const TAG_AT_END = /(?:^|\s)tag:(?:"([^"]*)|([^\s"]*))$/i;

/** Detects `tag:par` (or `tag:"par`) as the last word of the query, otherwise `null`. */
export function activeTagToken(query: string): TagToken | null {
  const m = TAG_AT_END.exec(query);
  if (!m) return null;
  const start = m.index + m[0].toLowerCase().lastIndexOf("tag:");
  return { start, partial: m[1] ?? m[2] ?? "" };
}

/** `tag:name`, or `tag:"two words"` when the name contains spaces. */
export function formatTagFilter(name: string): string {
  return /\s/.test(name) ? `tag:"${name}"` : `tag:${name}`;
}

/** Replaces the half-typed `tag:...` with the chosen tag and leaves room to type on. */
export function applyTagSuggestion(query: string, token: TagToken, name: string): string {
  return `${query.slice(0, token.start)}${formatTagFilter(name)} `;
}

/** Tag names containing the typed text, shortest first (so `places` comes before `places/cities`). */
export function matchTagNames(names: readonly string[], partial: string, limit = 8): string[] {
  const needle = partial.trim().toLowerCase();
  return names
    .filter((n) => n.toLowerCase().includes(needle))
    .sort((a, b) => a.length - b.length || a.localeCompare(b))
    .slice(0, limit);
}
