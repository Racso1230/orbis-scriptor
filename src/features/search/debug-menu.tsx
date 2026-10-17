import { Database, FlaskConical, Settings } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { SEED_COUNT, SEED_EVENT_COUNT, useIndexMaintenance } from "./use-index-maintenance";

/** Gear menu in the header with recovery and dev tools. */
export function DebugMenu() {
  const { rebuildIndex, seedEntities, seedEvents, status, busy } = useIndexMaintenance();

  return (
    <div className="flex items-center gap-1">
      {status && (
        <span
          role="status"
          className={
            status.kind === "error" ? "text-xs text-destructive" : "text-xs text-muted-foreground"
          }
        >
          {status.message}
        </span>
      )}
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="icon" aria-label="Settings and debug menu">
            <Settings />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuLabel>Maintenance</DropdownMenuLabel>
          <DropdownMenuItem disabled={busy} onSelect={() => void rebuildIndex()}>
            <Database /> Rebuild search index
          </DropdownMenuItem>
          {import.meta.env.DEV && (
            <>
              <DropdownMenuSeparator />
              <DropdownMenuItem disabled={busy} onSelect={() => void seedEntities()}>
                <FlaskConical /> Seed {SEED_COUNT.toLocaleString("en-US")} test entities (dev)
              </DropdownMenuItem>
              <DropdownMenuItem disabled={busy} onSelect={() => void seedEvents()}>
                <FlaskConical /> Seed {SEED_EVENT_COUNT} test events (dev)
              </DropdownMenuItem>
            </>
          )}
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}
