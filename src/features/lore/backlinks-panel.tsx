import { EntityIcon } from "@/features/entities/entity-icon";
import { useEntityTypes } from "@/features/entities/queries";
import { useOpenEntity } from "@/features/entities/use-open-entity";
import { useBacklinks } from "./queries";

/** "Mentioned in" list on the entity page. */
export function BacklinksPanel({ entityId }: { entityId: string }) {
  const backlinks = useBacklinks(entityId);
  const types = useEntityTypes();
  const open = useOpenEntity();

  return (
    <section aria-label="Backlinks" className="flex flex-col gap-2">
      <h2 className="text-sm font-medium text-muted-foreground">
        Mentioned in{backlinks.data ? ` (${backlinks.data.length})` : ""}
      </h2>
      {backlinks.data?.length === 0 && (
        <p className="text-sm text-muted-foreground">
          Nothing mentions this yet. Type <code>[[</code> in another entity's lore to link here.
        </p>
      )}
      <ul className="flex flex-col gap-1">
        {backlinks.data?.map((link) => {
          const type = types.data?.find((t) => t.key === link.sourceType);
          return (
            <li key={link.sourceId}>
              <button
                type="button"
                onClick={() => open(link.sourceId)}
                className="flex w-full flex-col items-start gap-0.5 rounded-md px-3 py-2 text-left hover:bg-accent"
              >
                <span className="flex items-center gap-2 text-sm font-medium">
                  <EntityIcon icon={type?.icon ?? ""} className="size-4 text-muted-foreground" />
                  {link.sourceName}
                </span>
                {link.snippet && (
                  <span className="text-xs text-muted-foreground">{link.snippet}</span>
                )}
              </button>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
