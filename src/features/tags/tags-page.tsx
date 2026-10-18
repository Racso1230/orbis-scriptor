import { useState } from "react";
import { Trash2 } from "lucide-react";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { EntityAvatar } from "@/features/entities/entity-avatar";
import { useEntityTypes } from "@/features/entities/queries";
import { useOpenEntity } from "@/features/entities/use-open-entity";
import { ColorPicker } from "@/features/types/pickers";
import { DEFAULT_COLOR, isHexColor } from "@/features/types/draft";
import type { Tag } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { normalizeTagName, tagKey, validateTagName } from "./tag-utils";
import { useDeleteTag, useEntitiesByTag, useRenameTag, useSetTagColor, useTags } from "./queries";

/** Route component for `/world/tags`: all tags, with rename, colour, delete and their entities. */
export function TagsPage() {
  const tags = useTags();
  const [selectedId, setSelectedId] = useState<string | null>(null);

  if (tags.isPending) return <p className="p-6 text-sm text-muted-foreground">Loading…</p>;
  if (tags.error || !tags.data) {
    return (
      <p className="p-6 text-sm text-destructive">
        {tags.error?.message ?? "Could not load the tags."}
      </p>
    );
  }
  const list = tags.data;
  const selected = list.find((t) => t.id === selectedId) ?? null;

  return (
    <div className="min-w-0 flex-1 overflow-auto">
      <div className="mx-auto flex max-w-5xl flex-col gap-6 p-6">
        <header>
          <h1 className="text-xl font-semibold">Tags</h1>
          <p className="mt-1 text-sm text-muted-foreground">
            Tags group entities across types. Use / for nested tags such as places/cities. Searching
            tag:places also finds everything below it.
          </p>
        </header>
        <div className="grid gap-6 md:grid-cols-[18rem_minmax(0,1fr)]">
          <section aria-label="All tags">
            {list.length === 0 ? (
              <p className="text-sm text-muted-foreground">
                No tags yet. Add some on an entity page.
              </p>
            ) : (
              <ul className="flex flex-col gap-1">
                {list.map((tag) => (
                  <li key={tag.id}>
                    <button
                      type="button"
                      onClick={() => setSelectedId(tag.id)}
                      aria-current={tag.id === selected?.id ? "true" : undefined}
                      className={cn(
                        "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent",
                        tag.id === selected?.id && "bg-accent font-medium",
                      )}
                    >
                      <span
                        aria-hidden
                        className="size-2.5 shrink-0 rounded-full bg-muted-foreground/50"
                        style={tag.color ? { backgroundColor: tag.color } : undefined}
                      />
                      <span className="truncate">{tag.name}</span>
                      <span className="ml-auto text-xs text-muted-foreground tabular-nums">
                        {tag.count}
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </section>
          <section aria-label="Tag details" className="min-w-0">
            {selected ? (
              <TagDetails
                key={selected.id}
                tag={selected}
                all={list}
                onDeleted={() => setSelectedId(null)}
              />
            ) : (
              <p className="text-sm text-muted-foreground">Choose a tag to manage it.</p>
            )}
          </section>
        </div>
      </div>
    </div>
  );
}

function TagDetails({ tag, all, onDeleted }: { tag: Tag; all: Tag[]; onDeleted: () => void }) {
  const rename = useRenameTag();
  const setColor = useSetTagColor();
  const remove = useDeleteTag();
  const entities = useEntitiesByTag([tag.name], false);
  const types = useEntityTypes();
  const open = useOpenEntity();

  const [name, setName] = useState(tag.name);
  const [color, setColorDraft] = useState(tag.color ?? DEFAULT_COLOR);
  const [error, setError] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const cleaned = normalizeTagName(name);
  const clash = all.find((t) => t.id !== tag.id && tagKey(t.name) === tagKey(cleaned));
  const unchanged = cleaned === tag.name;

  async function handleRename() {
    const problem = validateTagName(cleaned);
    if (problem) return setError(problem);
    setError(null);
    try {
      const surviving = await rename.mutateAsync({ id: tag.id, name: cleaned });
      setName(surviving.name);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function saveColor(next: string | null) {
    setError(null);
    try {
      await setColor.mutateAsync({ id: tag.id, color: next });
      if (next === null) setColorDraft(DEFAULT_COLOR);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function handleDelete() {
    try {
      await remove.mutateAsync(tag.id);
      onDeleted();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  return (
    <div className="flex flex-col gap-5">
      <div className="flex flex-col gap-2">
        <Label htmlFor="tag-name">Name</Label>
        <div className="flex items-center gap-2">
          <Input
            id="tag-name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && !unchanged && void handleRename()}
            className="max-w-xs"
          />
          <Button
            variant="outline"
            disabled={unchanged || cleaned === "" || rename.isPending}
            onClick={() => void handleRename()}
          >
            {clash ? "Merge" : "Rename"}
          </Button>
        </div>
        {clash && !unchanged && (
          <p role="status" className="text-xs text-amber-700 dark:text-amber-400">
            A tag named “{clash.name}” already exists. Renaming merges these {tag.count} entities
            into it, and this tag disappears. Nested tags below it are not renamed.
          </p>
        )}
      </div>

      <div className="flex flex-col gap-2">
        <Label>Colour</Label>
        <ColorPicker value={color} onChange={setColorDraft} />
        <div className="flex gap-2">
          <Button
            variant="outline"
            size="sm"
            disabled={!isHexColor(color) || setColor.isPending}
            onClick={() => void saveColor(color.toLowerCase())}
          >
            Save colour
          </Button>
          <Button
            variant="ghost"
            size="sm"
            disabled={tag.color === null || setColor.isPending}
            onClick={() => void saveColor(null)}
          >
            Clear colour
          </Button>
        </div>
      </div>

      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}

      <div className="flex flex-col gap-2">
        <h2 className="text-sm font-medium text-muted-foreground">
          Entities ({entities.data?.length ?? tag.count})
        </h2>
        <ul className="flex flex-col gap-1">
          {(entities.data ?? []).map((e) => {
            const type = types.data?.find((t) => t.key === e.type);
            return (
              <li key={e.id}>
                <button
                  type="button"
                  onClick={() => open(e.id)}
                  className="flex w-full items-center gap-2 rounded-md px-2 py-1 text-left text-sm hover:bg-accent"
                >
                  <EntityAvatar type={type} portrait={e.portrait} className="size-5" />
                  <span className="truncate">{e.name}</span>
                  <span className="ml-auto text-xs text-muted-foreground">
                    {type?.label ?? e.type}
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      </div>

      <div>
        <Button variant="destructive" onClick={() => setConfirmDelete(true)}>
          <Trash2 /> Delete tag
        </Button>
      </div>
      <AlertDialog open={confirmDelete} onOpenChange={setConfirmDelete}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete tag “{tag.name}”?</AlertDialogTitle>
            <AlertDialogDescription>
              The tag is removed from its {tag.count} {tag.count === 1 ? "entity" : "entities"}. The
              entities themselves are kept.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction onClick={() => void handleDelete()}>Delete</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
