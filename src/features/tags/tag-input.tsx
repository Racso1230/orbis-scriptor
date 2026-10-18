import { useId, useState } from "react";
import { Input } from "@/components/ui/input";
import {
  MAX_TAGS_PER_ENTITY,
  normalizeTagName,
  suggestTags,
  tagKey,
  validateTagName,
} from "./tag-utils";
import { TagChip } from "./tag-chip";

interface Props {
  value: string[];
  /** Names of every tag in the world, for autocomplete. */
  known: string[];
  /** Lower-case name to `#rrggbb`, for chip colours. */
  colors?: ReadonlyMap<string, string>;
  /** Called with the complete new list. May reject: the message is shown below the field. */
  onChange: (names: string[]) => Promise<unknown> | void;
}

/**
 * Chips plus a text field. Enter or a comma adds the typed tag, a leading `#`
 * is dropped, and the x on a chip removes it. Suggestions come from `known`.
 */
export function TagInput({ value, known, colors, onChange }: Props) {
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);
  const listId = useId();

  const suggestions = draft.trim() === "" ? [] : suggestTags(known, draft, value);

  async function commit(next: string[]) {
    setError(null);
    try {
      await onChange(next);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      return false;
    }
    return true;
  }

  /** Adds one or several typed names in a single save (pasting "a, b" adds both). */
  async function add(...raws: string[]) {
    let next = value;
    for (const raw of raws) {
      const name = normalizeTagName(raw);
      if (name === "") continue;
      const problem = validateTagName(name);
      if (problem) {
        setError(problem);
        return;
      }
      if (next.some((t) => tagKey(t) === tagKey(name))) continue;
      if (next.length >= MAX_TAGS_PER_ENTITY) {
        setError(`An entity can have at most ${MAX_TAGS_PER_ENTITY} tags.`);
        return;
      }
      // Reuse the spelling of an existing tag (names are case-insensitive).
      const existing = known.find((k) => tagKey(k) === tagKey(name));
      next = [...next, existing ?? name];
    }
    if (next === value) {
      setDraft("");
      setError(null);
      return;
    }
    if (await commit(next)) setDraft("");
  }

  function onKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      void add(draft);
    } else if (e.key === "Backspace" && draft === "" && value.length > 0) {
      void commit(value.slice(0, -1));
    }
  }

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex flex-wrap items-center gap-1.5" role="list" aria-label="Tags">
        {value.map((name) => (
          <div role="listitem" key={name} className="max-w-full">
            <TagChip
              name={name}
              color={colors?.get(tagKey(name))}
              onRemove={() => void commit(value.filter((t) => t !== name))}
            />
          </div>
        ))}
        <div className="relative min-w-32 flex-1">
          <Input
            value={draft}
            onChange={(e) => {
              // A comma only arrives here when text is pasted ("a, b").
              const text = e.target.value;
              if (text.includes(",")) {
                void add(...text.split(","));
              } else {
                setDraft(text);
                setError(null);
              }
            }}
            onKeyDown={onKeyDown}
            placeholder="Add tag…"
            aria-label="Add tag"
            aria-invalid={error !== null}
            aria-controls={suggestions.length > 0 ? listId : undefined}
            className="h-7 text-xs"
          />
          {suggestions.length > 0 && (
            <ul
              id={listId}
              role="listbox"
              aria-label="Tag suggestions"
              className="absolute top-full left-0 z-20 mt-1 max-h-48 w-full min-w-40 overflow-auto rounded-md border bg-popover p-1 text-xs shadow-md"
            >
              {suggestions.map((name) => (
                <li key={name} role="option" aria-selected={false}>
                  <button
                    type="button"
                    // mouseDown keeps the input focused, so the click is not lost to a blur.
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => void add(name)}
                    className="w-full rounded-sm px-2 py-1 text-left hover:bg-accent"
                  >
                    {name}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
      {error && (
        <p role="alert" className="text-xs text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}
