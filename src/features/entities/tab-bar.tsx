import { useNavigate, useParams } from "@tanstack/react-router";
import { X } from "lucide-react";
import { useCurrentWorld } from "@/features/worlds/queries";
import { cn } from "@/lib/utils";
import { useWorldTabs } from "@/stores/tabs-store";
import { EntityIcon } from "./entity-icon";
import { useEntities, useEntityTypes } from "./queries";
import { useCloseTab } from "./use-close-tab";

/** Open entity tabs for the current world. The active tab follows the route. */
export function TabBar() {
  const navigate = useNavigate();
  const { id: activeId } = useParams({ strict: false });
  const world = useCurrentWorld();
  const tabs = useWorldTabs(world.data?.path);
  const closeTab = useCloseTab();
  const entities = useEntities();
  const types = useEntityTypes();

  if (tabs.length === 0) return null;

  function go(id: string) {
    void navigate({ to: "/world/entity/$id", params: { id } });
  }

  return (
    <div role="tablist" className="flex h-9 shrink-0 items-stretch overflow-x-auto border-b">
      {tabs.map((id) => {
        const entity = entities.data?.find((e) => e.id === id);
        const type = types.data?.find((t) => t.key === entity?.type);
        const active = id === activeId;
        return (
          <div
            key={id}
            role="tab"
            aria-selected={active}
            className={cn(
              "group flex max-w-48 items-center gap-1 border-r pr-1 pl-3 text-sm",
              active ? "bg-background" : "bg-muted/40 text-muted-foreground hover:bg-muted",
            )}
          >
            <button
              type="button"
              onClick={() => go(id)}
              className="flex min-w-0 items-center gap-2"
            >
              <EntityIcon icon={type?.icon ?? ""} className="size-3.5 shrink-0" />
              <span className="truncate">{entity?.name ?? "…"}</span>
            </button>
            <button
              type="button"
              onClick={() => closeTab(id)}
              aria-label={`Close ${entity?.name ?? "tab"}`}
              className="rounded-sm p-0.5 opacity-0 group-hover:opacity-100 hover:bg-accent focus:opacity-100"
            >
              <X className="size-3.5" />
            </button>
          </div>
        );
      })}
    </div>
  );
}
