import { useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { ChevronDown, ChevronRight, Tags, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { TagFilter } from "@/features/entities/sidebar-rows";
import { cn } from "@/lib/utils";
import { useTags } from "./queries";
import { TagChip } from "./tag-chip";

/**
 * Collapsible tag filter above the sidebar list: click tags to select them,
 * choose whether an entity needs any or all of them. Filtering itself happens
 * in `buildSidebarRows`, on the tags already present in the entity list.
 */
export function SidebarTagFilter({
  value,
  onChange,
}: {
  value: TagFilter;
  onChange: (next: TagFilter) => void;
}) {
  const tags = useTags();
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);
  const all = tags.data ?? [];
  if (all.length === 0 && value.tags.length === 0) return null;

  const Chevron = open ? ChevronDown : ChevronRight;
  function toggle(name: string) {
    const has = value.tags.includes(name);
    onChange({
      ...value,
      tags: has ? value.tags.filter((t) => t !== name) : [...value.tags, name],
    });
  }

  return (
    <div className="px-2 pb-2">
      <div className="flex items-center gap-1">
        <button
          type="button"
          aria-expanded={open}
          onClick={() => setOpen(!open)}
          className="flex flex-1 items-center gap-1 rounded-md px-1 py-0.5 text-xs font-medium tracking-wide text-muted-foreground uppercase hover:bg-accent"
        >
          <Chevron className="size-3.5" />
          <Tags className="size-3.5" />
          Filter by tag
          {value.tags.length > 0 && (
            <span className="ml-auto rounded-full bg-primary px-1.5 text-primary-foreground normal-case">
              {value.tags.length}
            </span>
          )}
        </button>
        {value.tags.length > 0 && (
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label="Clear tag filter"
            onClick={() => onChange({ ...value, tags: [] })}
          >
            <X />
          </Button>
        )}
      </div>
      {open && (
        <div className="mt-1 flex flex-col gap-2">
          <div
            role="group"
            aria-label="Match"
            className="inline-flex w-fit overflow-hidden rounded-md border text-xs"
          >
            {[
              { all: false, label: "Any" },
              { all: true, label: "All" },
            ].map((opt) => (
              <button
                key={opt.label}
                type="button"
                aria-pressed={value.matchAll === opt.all}
                onClick={() => onChange({ ...value, matchAll: opt.all })}
                className={cn(
                  "px-2 py-0.5 hover:bg-accent",
                  value.matchAll === opt.all && "bg-accent font-medium",
                )}
              >
                {opt.label}
              </button>
            ))}
          </div>
          <div
            className="flex max-h-32 flex-wrap gap-1 overflow-auto"
            role="group"
            aria-label="Tags"
          >
            {all.map((tag) => (
              <TagChip
                key={tag.id}
                name={tag.name}
                color={tag.color}
                selected={value.tags.includes(tag.name)}
                onClick={() => toggle(tag.name)}
              />
            ))}
          </div>
          <button
            type="button"
            className="w-fit text-xs text-muted-foreground underline-offset-2 hover:underline"
            onClick={() => void navigate({ to: "/world/tags" })}
          >
            Manage tags…
          </button>
        </div>
      )}
    </div>
  );
}
