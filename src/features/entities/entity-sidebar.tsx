import { useMemo, useRef, useState } from "react";
import { useNavigate, useParams, useRouterState } from "@tanstack/react-router";
import { useVirtualizer } from "@tanstack/react-virtual";
import { CalendarDays, ChevronDown, ChevronRight, Network, Plus, Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useCurrentWorld } from "@/features/worlds/queries";
import { cn } from "@/lib/utils";
import { useTabsStore } from "@/stores/tabs-store";
import { EntityIcon } from "./entity-icon";
import { lookFor } from "./type-look";
import { useCreateEntity, useEntities, useEntityTypes } from "./queries";
import { buildSidebarRows } from "./sidebar-rows";

const ROW_HEIGHT = 32;

/** Entities grouped by type, with filter box and per-type "new" buttons. */
export function EntitySidebar() {
  const navigate = useNavigate();
  const { id: activeId } = useParams({ strict: false });
  const world = useCurrentWorld();
  const types = useEntityTypes();
  const entities = useEntities();
  const createEntity = useCreateEntity();
  const openTab = useTabsStore((s) => s.openTab);
  const onTimeline = useRouterState({ select: (s) => s.location.pathname.endsWith("/timeline") });
  const onGraph = useRouterState({ select: (s) => s.location.pathname.endsWith("/graph") });

  const [filter, setFilter] = useState("");
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());

  const rows = useMemo(
    () => buildSidebarRows(types.data ?? [], entities.data ?? [], filter, collapsed),
    [types.data, entities.data, filter, collapsed],
  );

  // Only the rows in view are rendered, so thousands of entities stay fast.
  const parentRef = useRef<HTMLDivElement>(null);
  // eslint-disable-next-line react-hooks/incompatible-library -- we do not use the React Compiler
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 10,
  });

  function toggle(typeKey: string) {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(typeKey)) next.delete(typeKey);
      else next.add(typeKey);
      return next;
    });
  }

  function open(entityId: string) {
    if (world.data) openTab(world.data.path, entityId);
    void navigate({ to: "/world/entity/$id", params: { id: entityId } });
  }

  async function handleNew(typeKey: string, label: string) {
    const entity = await createEntity.mutateAsync({ type: typeKey, name: `New ${label}` });
    open(entity.id);
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex flex-col gap-1 px-2 pt-2">
        <Button
          variant={onTimeline ? "secondary" : "ghost"}
          size="sm"
          className="w-full justify-start"
          aria-current={onTimeline ? "page" : undefined}
          onClick={() => void navigate({ to: "/world/timeline" })}
        >
          <CalendarDays /> Timeline
        </Button>
        <Button
          variant={onGraph ? "secondary" : "ghost"}
          size="sm"
          className="w-full justify-start"
          aria-current={onGraph ? "page" : undefined}
          onClick={() => void navigate({ to: "/world/graph" })}
        >
          <Network /> Graph
        </Button>
      </div>

      <div className="relative p-2">
        <Search className="pointer-events-none absolute top-1/2 left-4 size-4 -translate-y-1/2 text-muted-foreground" />
        <Input
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          placeholder="Filter entities…"
          className="pl-8"
          aria-label="Filter entities"
        />
      </div>

      <div ref={parentRef} className="flex-1 overflow-auto px-2 pb-2">
        <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
          {virtualizer.getVirtualItems().map((item) => {
            const row = rows[item.index];
            if (!row) return null;
            const style = {
              position: "absolute" as const,
              top: 0,
              left: 0,
              width: "100%",
              height: item.size,
              transform: `translateY(${item.start}px)`,
            };

            if (row.kind === "header") {
              const Chevron = row.collapsed ? ChevronRight : ChevronDown;
              return (
                <div key={item.key} style={style} className="group flex items-center gap-1">
                  <button
                    type="button"
                    onClick={() => toggle(row.type.key)}
                    className="flex min-w-0 flex-1 items-center gap-1 rounded-md px-1 text-xs font-medium tracking-wide text-muted-foreground uppercase hover:bg-accent"
                    aria-expanded={!row.collapsed}
                  >
                    <Chevron className="size-3.5 shrink-0" />
                    <EntityIcon
                      icon={row.type.icon}
                      className="size-3.5 shrink-0"
                      style={{ color: lookFor(row.type).color }}
                    />
                    <span className="truncate">{row.type.plural}</span>
                    <span className="ml-auto font-normal tabular-nums">{row.count}</span>
                  </button>
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    aria-label={`New ${row.type.label}`}
                    onClick={() => handleNew(row.type.key, row.type.label)}
                    disabled={createEntity.isPending}
                  >
                    <Plus />
                  </Button>
                </div>
              );
            }

            const type = types.data?.find((t) => t.key === row.entity.type);
            const active = row.entity.id === activeId;
            return (
              <button
                key={item.key}
                type="button"
                style={style}
                onClick={() => open(row.entity.id)}
                className={cn(
                  "flex items-center gap-2 rounded-md px-2 text-sm hover:bg-accent",
                  active && "bg-accent font-medium",
                )}
              >
                <EntityIcon
                  icon={type?.icon ?? ""}
                  className="size-4 shrink-0"
                  style={{ color: lookFor(type).color }}
                />
                <span className="truncate">{row.entity.name}</span>
              </button>
            );
          })}
        </div>
        {entities.data?.length === 0 && (
          <p className="px-2 py-4 text-sm text-muted-foreground">
            No entities yet. Use the + next to a type, or press Ctrl+N.
          </p>
        )}
      </div>
    </div>
  );
}
