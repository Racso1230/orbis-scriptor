import { useState } from "react";
import {
  CommandDialog,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  Command,
  CommandEmpty,
} from "@/components/ui/command";
import { BlankMapDialog } from "@/features/maps/blank-map-dialog";
import { EntityIcon } from "@/features/entities/entity-icon";
import { lookFor } from "@/features/entities/type-look";
import { useEntityTypes } from "@/features/entities/queries";
import { useCreateAndOpen } from "@/features/entities/use-create-and-open";
import { useOpenEntity } from "@/features/entities/use-open-entity";

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
  const openEntity = useOpenEntity();
  const [blankOpen, setBlankOpen] = useState(false);

  return (
    <>
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
                  <EntityIcon icon={t.icon} style={{ color: lookFor(t).color }} />
                  {t.label}
                </CommandItem>
              ))}
              <CommandItem
                value="Blank map"
                onSelect={() => {
                  onOpenChange(false);
                  setBlankOpen(true);
                }}
              >
                <EntityIcon icon={types.data?.find((t) => t.key === "map")?.icon ?? ""} />
                Blank map�
              </CommandItem>
            </CommandGroup>
          </CommandList>
        </Command>
      </CommandDialog>
      <BlankMapDialog
        open={blankOpen}
        onOpenChange={setBlankOpen}
        target={{ mode: "create", onCreated: openEntity }}
      />
    </>
  );
}
