import { Outlet } from "@tanstack/react-router";
import { ThemeToggle } from "@/components/theme-toggle";

/** Top bar plus the routed page below it. */
export function AppShell() {
  return (
    <div className="flex h-screen flex-col">
      <header className="flex h-12 shrink-0 items-center justify-between border-b px-4">
        <span className="font-semibold">Orbis Scriptor</span>
        <ThemeToggle />
      </header>
      <main className="flex flex-1 overflow-hidden">
        <Outlet />
      </main>
    </div>
  );
}
