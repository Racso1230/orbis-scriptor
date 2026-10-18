import { useState } from "react";
import { FolderOpen } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { MarkdownExportOptions } from "@/lib/bindings";
import { PHASE_LABELS, progressFraction } from "./export-state";
import { startMarkdownExport, useExportStore } from "./export-store";
import { pickExportFolder } from "./pick-export-folder";

/** Mounted once in the world layout; opened from the gear menu and the palette. */
export function ExportMarkdownDialog() {
  const open = useExportStore((s) => s.dialogOpen);
  const setOpen = useExportStore((s) => s.setDialogOpen);
  const state = useExportStore((s) => s.state);

  const [folder, setFolder] = useState<string | null>(null);
  const [options, setOptions] = useState<MarkdownExportOptions>({
    includeAssets: true,
    includeMaps: true,
    includeEvents: true,
    overwrite: false,
  });

  const running = state.status === "running";
  const fraction = progressFraction(state);

  function toggle(key: keyof MarkdownExportOptions, on: boolean) {
    setOptions((o) => ({ ...o, [key]: on }));
  }

  async function choose() {
    const picked = await pickExportFolder();
    if (picked) setFolder(picked);
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Export to Markdown</DialogTitle>
          <DialogDescription>
            Writes one note per entity with links that Obsidian understands. The world folder is
            created inside the folder you choose.
          </DialogDescription>
        </DialogHeader>

        <fieldset disabled={running} className="flex flex-col gap-2">
          <legend className="sr-only">Export options</legend>
          <Option
            label="Include map images"
            checked={options.includeAssets}
            onChange={(on) => toggle("includeAssets", on)}
          />
          <Option
            label="Include maps"
            checked={options.includeMaps}
            onChange={(on) => toggle("includeMaps", on)}
          />
          <Option
            label="Include events"
            checked={options.includeEvents}
            onChange={(on) => toggle("includeEvents", on)}
          />
          <Option
            label="Replace an earlier export of this world"
            checked={options.overwrite}
            onChange={(on) => toggle("overwrite", on)}
          />
          <div className="mt-1 flex items-center gap-2">
            <Button variant="outline" size="sm" onClick={() => void choose()}>
              <FolderOpen /> Choose folder…
            </Button>
            <span className="min-w-0 truncate text-sm text-muted-foreground" title={folder ?? ""}>
              {folder ?? "No folder chosen"}
            </span>
          </div>
        </fieldset>

        {state.status !== "idle" && (
          <div className="flex flex-col gap-2" aria-live="polite">
            {running && (
              <>
                <div
                  role="progressbar"
                  aria-label="Export progress"
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuenow={fraction === null ? undefined : Math.round(fraction * 100)}
                  className="h-2 w-full overflow-hidden rounded-full bg-muted"
                >
                  <div
                    className={
                      fraction === null
                        ? "h-full w-1/3 animate-pulse bg-primary"
                        : "h-full bg-primary transition-all"
                    }
                    style={fraction === null ? undefined : { width: `${fraction * 100}%` }}
                  />
                </div>
                <p className="text-sm text-muted-foreground">
                  {PHASE_LABELS[state.phase] ?? state.phase}
                  {state.total > 0 && ` ${state.done} / ${state.total}`}
                </p>
              </>
            )}
            {state.status === "done" && (
              <p role="status" className="text-sm">
                Exported {state.files} files to <code className="break-all">{state.path}</code>
              </p>
            )}
            {state.status === "failed" && (
              <p role="alert" className="text-sm text-destructive">
                {state.error}
              </p>
            )}
          </div>
        )}

        <DialogFooter>
          <Button variant="outline" onClick={() => setOpen(false)}>
            {running ? "Hide" : "Close"}
          </Button>
          <Button
            disabled={running || folder === null}
            onClick={() => folder && void startMarkdownExport(folder, options)}
          >
            {state.status === "failed" ? "Try again" : "Export"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function Option({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (on: boolean) => void;
}) {
  return (
    <label className="flex items-center gap-2 text-sm">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      {label}
    </label>
  );
}
