import { Map as MapIcon } from "lucide-react";
import { useOpenEntity } from "@/features/entities/use-open-entity";
import { useMapToolStore } from "./map-tool-store";
import { useEntityMapAppearances } from "./queries";

/** "Appears on maps" list on an entity page; each row jumps to the pin. */
export function AppearsOnMaps({ entityId }: { entityId: string }) {
  const appearances = useEntityMapAppearances(entityId);
  const open = useOpenEntity();
  const setPendingFocus = useMapToolStore((s) => s.setPendingFocus);

  return (
    <section aria-label="Appears on maps" className="flex flex-col gap-2">
      <h2 className="text-sm font-medium text-muted-foreground">
        Appears on maps{appearances.data ? ` (${appearances.data.length})` : ""}
      </h2>
      {appearances.data?.length === 0 && (
        <p className="text-sm text-muted-foreground">
          Not pinned on any map yet. Use the Pin tool on a map to place it.
        </p>
      )}
      <ul className="flex flex-col gap-1">
        {appearances.data?.map((a) => (
          <li key={a.objectId}>
            <button
              type="button"
              onClick={() => {
                // The map view consumes this once it has loaded and centres on the pin.
                setPendingFocus({ mapEntityId: a.mapEntityId, objectId: a.objectId });
                open(a.mapEntityId);
              }}
              className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm hover:bg-accent"
            >
              <MapIcon className="size-4 text-muted-foreground" />
              {a.mapName}
              <span className="ml-auto text-xs text-muted-foreground">Show on map</span>
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
