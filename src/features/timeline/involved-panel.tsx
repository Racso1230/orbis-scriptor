import { useState } from "react";
import { Plus, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { EntityIcon } from "@/features/entities/entity-icon";
import { useCreateEntity, useEntityTypes } from "@/features/entities/queries";
import { useOpenEntity } from "@/features/entities/use-open-entity";
import { EntityPickerDialog } from "@/features/maps/entity-picker-dialog";
import { useAddInvolved, useInvolved, useRemoveInvolved } from "./queries";

/** "Involved" section of an event page: who/what took part, with add and remove. */
export function InvolvedPanel({ eventId }: { eventId: string }) {
  const involved = useInvolved(eventId);
  const addInvolved = useAddInvolved(eventId);
  const removeInvolved = useRemoveInvolved(eventId);
  const createEntity = useCreateEntity();
  const types = useEntityTypes();
  const open = useOpenEntity();
  const [picking, setPicking] = useState(false);

  async function handlePick(entityId: string) {
    setPicking(false);
    // Adding someone twice is harmless to the user, so skip the round trip.
    if (involved.data?.some((e) => e.id === entityId)) return;
    await addInvolved.mutateAsync(entityId);
  }

  async function handleCreate(type: string, name: string) {
    setPicking(false);
    const entity = await createEntity.mutateAsync({ type, name });
    await addInvolved.mutateAsync(entity.id);
  }

  const error = addInvolved.error ?? removeInvolved.error;

  return (
    <section aria-label="Involved" className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-medium text-muted-foreground">
          Involved{involved.data ? ` (${involved.data.length})` : ""}
        </h2>
        <Button variant="ghost" size="sm" onClick={() => setPicking(true)}>
          <Plus /> Add
        </Button>
      </div>
      {involved.data?.length === 0 && (
        <p className="text-sm text-muted-foreground">
          Nobody linked yet. Add the characters, places and factions that took part.
        </p>
      )}
      <ul className="flex flex-col gap-1">
        {involved.data?.map((entity) => {
          const type = types.data?.find((t) => t.key === entity.type);
          return (
            <li key={entity.id} className="group flex items-center gap-1">
              <button
                type="button"
                onClick={() => open(entity.id)}
                className="flex min-w-0 flex-1 items-center gap-2 rounded-md px-3 py-2 text-left text-sm hover:bg-accent"
              >
                <EntityIcon
                  icon={type?.icon ?? ""}
                  className="size-4 shrink-0 text-muted-foreground"
                />
                <span className="truncate">{entity.name}</span>
                <span className="ml-auto text-xs text-muted-foreground">
                  {type?.label ?? entity.type}
                </span>
              </button>
              <Button
                variant="ghost"
                size="icon-xs"
                aria-label={`Remove ${entity.name}`}
                disabled={removeInvolved.isPending}
                onClick={() => removeInvolved.mutate(entity.id)}
              >
                <X />
              </Button>
            </li>
          );
        })}
      </ul>
      {error && <p className="text-sm text-destructive">{error.message}</p>}
      <EntityPickerDialog
        open={picking}
        onOpenChange={setPicking}
        onPickEntity={(id) => void handlePick(id)}
        onCreateEntity={(type, name) => void handleCreate(type, name)}
        title="Add involved entity"
        description="Choose who or what took part in this event"
      />
    </section>
  );
}
