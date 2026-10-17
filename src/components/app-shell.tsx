import { Outlet, useNavigate } from "@tanstack/react-router";
import { X } from "lucide-react";
import { ThemeToggle } from "@/components/theme-toggle";
import { Button } from "@/components/ui/button";
import { DebugMenu } from "@/features/search/debug-menu";
import { useStartupUpdateCheck } from "@/features/updates/use-startup-update-check";
import { useCloseWorld, useCurrentWorld } from "@/features/worlds/queries";

/** Top bar plus the routed page below it. */
export function AppShell() {
  const navigate = useNavigate();
  const world = useCurrentWorld();
  const closeWorld = useCloseWorld();
  useStartupUpdateCheck();

  async function handleClose() {
    await closeWorld.mutateAsync();
    await navigate({ to: "/" });
  }

  return (
    <div className="flex h-screen flex-col">
      <header className="flex h-12 shrink-0 items-center justify-between border-b px-4">
        <div className="flex items-center gap-2">
          <span className="font-semibold">Orbis Scriptor</span>
          {world.data && (
            <>
              <span className="text-muted-foreground">/</span>
              <span className="truncate">{world.data.name}</span>
            </>
          )}
        </div>
        <div className="flex items-center gap-1">
          {world.data && <DebugMenu />}
          {world.data && (
            <Button variant="ghost" size="sm" onClick={handleClose} disabled={closeWorld.isPending}>
              <X /> Close world
            </Button>
          )}
          <ThemeToggle />
        </div>
      </header>
      <main className="flex flex-1 overflow-hidden">
        <Outlet />
      </main>
    </div>
  );
}
