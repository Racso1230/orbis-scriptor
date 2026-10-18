import { useState } from "react";
import { Plus } from "lucide-react";
import {
  CommandDialog,
  Command,
  CommandEmpty,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import { EntityIcon } from "@/features/entities/entity-icon";
import { useEntities, useEntityTypes } from "@/features/entities/queries";
import { buildMentionItems, type MentionItem } from "@/features/lore/mention-items";

function rank(item: MentionItem): number {
  if (item.kind === "entity") return 0;
  return item.type === "location" ? 1 : 2;
}

interface Props {
  open: boolean;
  title?: string;
  description?: string;
  /** Called with `false` on Escape/outside click: the pin is discarded. */
  onOpenChange: (open: boolean) => void;
  onPickEntity: (entityId: string) => void;
  onCreateEntity: (type: string, name: string) => void;
  /** Only offer (and create) entities of these types; empty or missing = any. */
  allowedTypes?: string[];
  /** Hide this entity from the matches (e.g. the one being edited). */
  excludeId?: string;
}

/**
 * Dialog shown after a click with the Pin tool: pick an existing entity or
 * create a new one. Reuses the `[[` mention picker's matching logic.
 */
export function EntityPickerDialog({
  open,
  onOpenChange,
  onPickEntity,
  onCreateEntity,
  title = "Link pin to an entity",
  description = "Choose an existing entity or create a new one for this pin",
  allowedTypes,
  excludeId,
}: Props) {
  const [query, setQuery] = useState("");
  const entities = useEntities();
  const types = useEntityTypes();

  // Keep entity matches first; among "create" rows put Location on top (the usual pin target).
  const restricted = allowedTypes !== undefined && allowedTypes.length > 0;
  const items = buildMentionItems(
    (entities.data ?? []).filter(
      (e) => e.id !== excludeId && (!restricted || allowedTypes.includes(e.type)),
    ),
    (types.data ?? []).filter((t) => !restricted || allowedTypes.includes(t.key)),
    query,
  ).sort((a, b) => rank(a) - rank(b));

  function handleOpenChange(next: boolean) {
    if (!next) setQuery("");
    onOpenChange(next);
  }

  return (
    <CommandDialog
      open={open}
      onOpenChange={handleOpenChange}
      title={title}
      description={description}
      className="sm:max-w-md"
    >
      {/* shouldFilter=false: buildMentionItems already filters and ranks. */}
      <Command shouldFilter={false}>
        <CommandInput
          value={query}
          onValueChange={setQuery}
          placeholder="Search entities, or type a name to create one…"
        />
        <CommandList>
          {items.length === 0 && <CommandEmpty>Type a name to create an entity.</CommandEmpty>}
          {items.map((item) =>
            item.kind === "entity" ? (
              <CommandItem
                key={`entity-${item.id}`}
                value={`entity-${item.id}`}
                onSelect={() => {
                  setQuery("");
                  onPickEntity(item.id);
                }}
              >
                <EntityIcon
                  icon={types.data?.find((t) => t.label === item.description)?.icon ?? ""}
                />
                <span className="truncate">{item.title}</span>
                <span className="ml-auto text-xs text-muted-foreground">{item.description}</span>
              </CommandItem>
            ) : (
              <CommandItem
                key={`create-${item.type}`}
                value={`create-${item.type}`}
                onSelect={() => {
                  setQuery("");
                  onCreateEntity(item.type, item.name);
                }}
              >
                <Plus />
                <span className="truncate">{item.title}</span>
              </CommandItem>
            ),
          )}
        </CommandList>
      </Command>
    </CommandDialog>
  );
}
