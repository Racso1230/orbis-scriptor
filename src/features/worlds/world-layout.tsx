import { Outlet } from "@tanstack/react-router";

/** Layout for everything inside an open world: sidebar + main area. */
export function WorldLayout() {
  return (
    <div className="flex flex-1 overflow-hidden">
      <aside className="flex w-64 shrink-0 flex-col border-r bg-sidebar p-3 text-sidebar-foreground">
        <p className="text-sm text-muted-foreground">Entities appear here in Phase 2.</p>
      </aside>
      <section className="flex flex-1 overflow-auto">
        <Outlet />
      </section>
    </div>
  );
}

/** Shown when no tab is open yet. */
export function WorldEmptyState() {
  return (
    <div className="flex flex-1 items-center justify-center text-muted-foreground">
      Your world is open. Entities, maps and timelines arrive in the next phases.
    </div>
  );
}
