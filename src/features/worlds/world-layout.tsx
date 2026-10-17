import { Outlet } from "@tanstack/react-router";
import { CommandPalette } from "@/features/search/command-palette";
import { EntitySidebar } from "@/features/entities/entity-sidebar";
import { TabBar } from "@/features/entities/tab-bar";

/** Layout for everything inside an open world: sidebar + tabs + main area. */
export function WorldLayout() {
  return (
    <div className="flex flex-1 overflow-hidden">
      <aside className="flex w-64 shrink-0 flex-col border-r bg-sidebar text-sidebar-foreground">
        <EntitySidebar />
      </aside>
      <section className="flex flex-1 flex-col overflow-hidden">
        <TabBar />
        <div className="flex flex-1 overflow-auto">
          <Outlet />
        </div>
      </section>
      <CommandPalette />
    </div>
  );
}

/** Shown when no tab is open yet. */
export function WorldEmptyState() {
  return (
    <div className="flex flex-1 items-center justify-center text-muted-foreground">
      Pick an entity in the sidebar, or add one with the + next to a type.
    </div>
  );
}
