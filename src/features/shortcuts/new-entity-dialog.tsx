import {
  CommandDialog,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  Command,
  CommandEmpty,
} from "@/components/ui/command";
import { EntityIcon } from "@/features/entities/entity-icon";
import { useEntityTypes } from "@/features/entities/queries";
import { useCreateAndOpen } from "@/features/entities/use-create-and-open";

/**
 * Ctrl+N: pick a type (type to filter, arrows + Enter) and a fresh entity of
 * that type is created and opened, ready to be renamed.
 */
export function NewEntityDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const types = useEntityTypes();
  const { createAndOpen } = useCreateAndOpen();

  return (
    <CommandDialog
      open={open}
      onOpenChange={onOpenChange}
      title="New entity"
      description="Choose what to create"
    >
      <Command>
        <CommandInput placeholder="New… (character, location, map)" />
        <CommandList>
          <CommandEmpty>No matching type.</CommandEmpty>
          <CommandGroup heading="Create new">
            {(types.data ?? []).map((t) => (
              <CommandItem
                key={t.key}
                value={t.label}
                onSelect={() => {
                  onOpenChange(false);
                  void createAndOpen(t.key, t.label);
                }}
              >
                <EntityIcon icon={t.icon} />
                {t.label}
              </CommandItem>
            ))}
          </CommandGroup>
        </CommandList>
      </Command>
    </CommandDialog>
  );
}
