import { useEffect, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import {
  CalendarCog,
  CalendarDays,
  Database,
  FileDown,
  FlaskConical,
  Network,
  Plus,
  Printer,
  RefreshCw,
  Shapes,
  Settings,
  X,
} from "lucide-react";
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import { EntityIcon } from "@/features/entities/entity-icon";
import { lookFor } from "@/features/entities/type-look";
import { useCreateEntity, useEntityTypes } from "@/features/entities/queries";
import { BlankMapDialog } from "@/features/maps/blank-map-dialog";
import { useOpenEntity } from "@/features/entities/use-open-entity";
import { runUpdateCheck } from "@/features/updates/check-for-updates";
import { useOpenGraph } from "@/features/relationships/use-open-graph";
import { useOpenTimeline } from "@/features/timeline/use-open-timeline";
import { useCloseWorld } from "@/features/worlds/queries";
import { useExportStore } from "@/features/export/export-store";
import { HighlightedSnippet } from "./highlighted-snippet";
import { useSearch } from "./queries";
import {
  SEED_COUNT,
  SEED_EVENT_COUNT,
  SEED_RELATIONSHIP_COUNT,
  useIndexMaintenance,
} from "./use-index-maintenance";

/**
 * Ctrl+K / Cmd+K palette: server-side search results plus quick actions.
 * Mounted inside the world layout, so it only exists while a world is open.
 */
export function CommandPalette() {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [blankOpen, setBlankOpen] = useState(false);
  const navigate = useNavigate();
  const openEntity = useOpenEntity();
  const openTimeline = useOpenTimeline();
  const openGraph = useOpenGraph();
  const types = useEntityTypes();
  const createEntity = useCreateEntity();
  const closeWorld = useCloseWorld();
  const search = useSearch(query);
  const maintenance = useIndexMaintenance();
  const openExport = useExportStore((s) => s.openDialog);

  function changeOpen(next: boolean) {
    setOpen(next);
    if (!next) setQuery("");
  }

  // Global shortcut. The listener lives on window so it works wherever focus is.
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        changeOpen(!open);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
    // changeOpen only calls state setters, so it is safe to leave out.
  }, [open]);

  const needle = query.trim().toLowerCase();
  const results = needle ? (search.data ?? []) : [];

  type Action = { id: string; label: string; icon: React.ReactNode; run: () => void };
  const actions: Action[] = [
    ...(types.data ?? []).map((t) => ({
      id: `new-${t.key}`,
      label: `New ${t.label}`,
      icon: <Plus />,
      run: () => {
        changeOpen(false);
        void createEntity
          .mutateAsync({ type: t.key, name: `New ${t.label}` })
          .then((entity) => openEntity(entity.id));
      },
    })),
    {
      id: "new-blank-map",
      label: "New blank map",
      icon: <Plus />,
      run: () => {
        changeOpen(false);
        setBlankOpen(true);
      },
    },
    {
      id: "open-timeline",
      label: "Open timeline",
      icon: <CalendarDays />,
      run: () => {
        changeOpen(false);
        openTimeline();
      },
    },
    {
      id: "open-graph",
      label: "Open graph",
      icon: <Network />,
      run: () => {
        changeOpen(false);
        openGraph();
      },
    },
    {
      id: "calendars",
      label: "Calendars",
      icon: <CalendarCog />,
      run: () => {
        changeOpen(false);
        void navigate({ to: "/world/calendars" });
      },
    },
    {
      id: "entity-types",
      label: "Entity types",
      icon: <Shapes />,
      run: () => {
        changeOpen(false);
        void navigate({ to: "/world/types" });
      },
    },
    {
      id: "export-markdown",
      label: "Export to Markdown…",
      icon: <FileDown />,
      run: () => {
        changeOpen(false);
        openExport();
      },
    },
    {
      id: "print-world",
      label: "Print world…",
      icon: <Printer />,
      run: () => {
        changeOpen(false);
        void navigate({ to: "/print/world" });
      },
    },
    {
      id: "settings",
      label: "Settings",
      icon: <Settings />,
      run: () => {
        changeOpen(false);
        void navigate({ to: "/settings" });
      },
    },
    {
      id: "check-updates",
      label: "Check for updates",
      icon: <RefreshCw />,
      run: () => {
        changeOpen(false);
        void runUpdateCheck({ manual: true });
      },
    },
    {
      id: "close-world",
      label: "Close world",
      icon: <X />,
      run: () => {
        changeOpen(false);
        void closeWorld.mutateAsync().then(() => navigate({ to: "/" }));
      },
    },
    {
      id: "rebuild-index",
      label: "Rebuild search index",
      icon: <Database />,
      // Stays open so the success/failure message below is visible.
      run: () => void maintenance.rebuildIndex(),
    },
    ...(import.meta.env.DEV
      ? [
          {
            id: "seed",
            label: `Seed ${SEED_COUNT.toLocaleString("en-US")} test entities (dev)`,
            icon: <FlaskConical />,
            run: () => void maintenance.seedEntities(),
          },
          {
            id: "seed-events",
            label: `Seed ${SEED_EVENT_COUNT} test events (dev)`,
            icon: <FlaskConical />,
            run: () => void maintenance.seedEvents(),
          },
          {
            id: "seed-relationships",
            label: `Seed ${SEED_RELATIONSHIP_COUNT} test relationships (dev)`,
            icon: <FlaskConical />,
            run: () => void maintenance.seedRelationships(),
          },
        ]
      : []),
  ];
  const visibleActions = actions.filter((a) => a.label.toLowerCase().includes(needle));

  return (
    <>
      <CommandDialog
        open={open}
        onOpenChange={changeOpen}
        title="Search"
        description="Search entities or run a command"
        className="sm:max-w-xl"
      >
        {/* shouldFilter=false: results are already filtered and ranked by the backend. */}
        <Command shouldFilter={false}>
          <CommandInput
            value={query}
            onValueChange={setQuery}
            placeholder="Search entities, or type a command…"
          />
          <CommandList>
            {results.length === 0 && visibleActions.length === 0 && (
              <CommandEmpty>No results.</CommandEmpty>
            )}
            {results.length > 0 && (
              <CommandGroup heading="Entities">
                {results.map((r) => {
                  const type = types.data?.find((t) => t.key === r.entityType);
                  return (
                    <CommandItem
                      key={r.entityId}
                      value={`entity-${r.entityId}`}
                      onSelect={() => {
                        changeOpen(false);
                        openEntity(r.entityId);
                      }}
                    >
                      <EntityIcon icon={type?.icon ?? ""} style={{ color: lookFor(type).color }} />
                      <div className="flex min-w-0 flex-col">
                        <span className="truncate">{r.name}</span>
                        {r.snippet && (
                          <span className="truncate text-xs text-muted-foreground">
                            <HighlightedSnippet snippet={r.snippet} />
                          </span>
                        )}
                      </div>
                    </CommandItem>
                  );
                })}
              </CommandGroup>
            )}
            {visibleActions.length > 0 && (
              <CommandGroup heading="Actions">
                {visibleActions.map((a) => (
                  <CommandItem key={a.id} value={`action-${a.id}`} onSelect={a.run}>
                    {a.icon}
                    {a.label}
                  </CommandItem>
                ))}
              </CommandGroup>
            )}
          </CommandList>
          {maintenance.status && (
            <p
              role="status"
              className={
                maintenance.status.kind === "error"
                  ? "border-t px-3 py-2 text-xs text-destructive"
                  : "border-t px-3 py-2 text-xs text-muted-foreground"
              }
            >
              {maintenance.status.message}
            </p>
          )}
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
