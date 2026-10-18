import { Outlet, useRouterState } from "@tanstack/react-router";
import { ErrorBoundary } from "@/components/error-boundary";
import { CalendarDays, Map, MapPin, User } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useEntities, useEntityTypes } from "@/features/entities/queries";
import { ExportMarkdownDialog } from "@/features/export/export-dialog";
import { useSyncTypeLooks } from "@/features/entities/type-look";
import { useCreateAndOpen } from "@/features/entities/use-create-and-open";
import { CommandPalette } from "@/features/search/command-palette";
import { EntitySidebar } from "@/features/entities/entity-sidebar";
import { ShortcutsHost } from "@/features/shortcuts/shortcuts-host";
import { TabBar } from "@/features/entities/tab-bar";

/** Layout for everything inside an open world: sidebar + tabs + main area. */
export function WorldLayout() {
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  useSyncTypeLooks();
  return (
    <div className="flex flex-1 overflow-hidden">
      <aside className="flex w-64 shrink-0 flex-col border-r bg-sidebar text-sidebar-foreground">
        <EntitySidebar />
      </aside>
      <section className="flex flex-1 flex-col overflow-hidden">
        <TabBar />
        <div className="flex flex-1 overflow-auto">
          {/* A crash on one page must not take the sidebar and tabs down with it. */}
          <ErrorBoundary inline resetKey={pathname}>
            <Outlet />
          </ErrorBoundary>
        </div>
      </section>
      <CommandPalette />
      <ShortcutsHost />
      <ExportMarkdownDialog />
    </div>
  );
}

/** The quick-start types offered in a brand-new world (in this order). */
const QUICK_START: { key: string; label: string; icon: typeof User }[] = [
  { key: "character", label: "character", icon: User },
  { key: "location", label: "location", icon: MapPin },
  { key: "map", label: "map", icon: Map },
  { key: "event", label: "event", icon: CalendarDays },
];

/** Shown when no tab is open yet. A world with no entities gets a welcome with quick actions. */
export function WorldEmptyState() {
  const entities = useEntities();
  const types = useEntityTypes();
  const { createAndOpen, isPending } = useCreateAndOpen();

  if (entities.isPending) return null;

  if (entities.data && entities.data.length > 0) {
    return (
      <div className="flex flex-1 items-center justify-center text-muted-foreground">
        Pick an entity in the sidebar, or add one with the + next to a type.
      </div>
    );
  }

  return (
    <div className="flex flex-1 items-center justify-center p-8">
      <div className="flex max-w-lg flex-col items-center gap-6 text-center">
        <div>
          <h1 className="text-2xl font-semibold">Your world is empty</h1>
          <p className="mt-1 text-muted-foreground">
            Start with the first thing that comes to mind. Everything you add can be linked with
            @mentions in lore.
          </p>
        </div>
        <div className="flex flex-wrap justify-center gap-2">
          {QUICK_START.map(({ key, label, icon: Icon }) => {
            const type = types.data?.find((t) => t.key === key);
            return (
              <Button
                key={key}
                variant="outline"
                disabled={!type || isPending}
                onClick={() => type && void createAndOpen(type.key, type.label)}
              >
                <Icon /> New {label}
              </Button>
            );
          })}
        </div>
        <p className="text-sm text-muted-foreground">
          Press{" "}
          <kbd className="rounded border bg-muted px-1.5 py-0.5 font-mono text-xs">Ctrl+K</kbd> to
          search or run commands, and{" "}
          <kbd className="rounded border bg-muted px-1.5 py-0.5 font-mono text-xs">Ctrl+N</kbd> to
          create anything.
        </p>
      </div>
    </div>
  );
}
