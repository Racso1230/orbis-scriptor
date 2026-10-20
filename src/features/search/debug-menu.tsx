import { useNavigate } from "@tanstack/react-router";
import {
  CalendarCog,
  Database,
  FlaskConical,
  FileDown,
  FileUp,
  Mountain,
  Printer,
  Settings,
  Shapes,
  Tags,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useExportStore } from "@/features/export/export-store";
import { useGenerateStore } from "@/features/generator/generate-store";
import { useImportStore } from "@/features/importer/import-store";
import {
  SEED_COUNT,
  SEED_EVENT_COUNT,
  SEED_RELATIONSHIP_COUNT,
  useIndexMaintenance,
} from "./use-index-maintenance";

/** Gear menu in the header with recovery and dev tools. */
export function DebugMenu() {
  const navigate = useNavigate();
  const openExport = useExportStore((s) => s.openDialog);
  const openImport = useImportStore((s) => s.openDialog);
  const openGenerate = useGenerateStore((s) => s.openDialog);
  const { rebuildIndex, seedEntities, seedEvents, seedRelationships, status, busy } =
    useIndexMaintenance();

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
          <DropdownMenuItem onSelect={() => void navigate({ to: "/settings" })}>
            <Settings /> Settings
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={() => void navigate({ to: "/world/calendars" })}>
            <CalendarCog /> Calendars
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={() => void navigate({ to: "/world/types" })}>
            <Shapes /> Entity types
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={() => void navigate({ to: "/world/tags" })}>
            <Tags /> Tags
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={openImport}>
            <FileUp /> Import Markdown / Obsidian vault…
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={openGenerate}>
            <Mountain /> Generate map…
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={openExport}>
            <FileDown /> Export to Markdown…
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={() => void navigate({ to: "/print/world" })}>
            <Printer /> Print world…
          </DropdownMenuItem>
          <DropdownMenuSeparator />
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
              <DropdownMenuItem disabled={busy} onSelect={() => void seedRelationships()}>
                <FlaskConical /> Seed {SEED_RELATIONSHIP_COUNT} test relationships (dev)
              </DropdownMenuItem>
            </>
          )}
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}
