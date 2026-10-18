import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import type { MapInfo, MapLayer, MapObject } from "@/lib/bindings";
import { toastError } from "@/lib/notify";
import { EXPORT_SCALES, clampExportSize, exportFileName } from "./export-png";
import { pickExportPath } from "./export-path";
import { renderMapToPng } from "./export-render";
import { useSaveExport } from "./queries";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  map: MapInfo;
  layers: MapLayer[];
  objects: MapObject[];
}

/** "Export PNG…": choose scale, hidden layers and pins, then pick a file and save. */
export function ExportDialog({ open, onOpenChange, map, layers, objects }: Props) {
  // Remount the body each time the dialog opens so it starts from the defaults.
  return (
    <Dialog open={open} onOpenChange={(next) => onOpenChange(next)}>
      <DialogContent className="sm:max-w-md">
        <ExportForm
          map={map}
          layers={layers}
          objects={objects}
          onClose={() => onOpenChange(false)}
        />
      </DialogContent>
    </Dialog>
  );
}

function ExportForm({
  map,
  layers,
  objects,
  onClose,
}: Omit<Props, "open" | "onOpenChange"> & { onClose: () => void }) {
  const [scale, setScale] = useState(1);
  const [includeHidden, setIncludeHidden] = useState(false);
  const [includePins, setIncludePins] = useState(true);
  const [busy, setBusy] = useState(false);
  const save = useSaveExport();

  const size = clampExportSize(map.width, map.height, scale);
  const hasHidden = layers.some((l) => !l.visible);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (busy) return;
    try {
      const path = await pickExportPath(exportFileName(map.name));
      if (!path) return; // cancelled
      setBusy(true);
      // Let the "Exporting…" state paint before the heavy, synchronous drawing starts.
      await new Promise((resolve) => setTimeout(resolve, 50));
      const pngBase64 = await renderMapToPng({
        map,
        layers,
        objects,
        options: { scale: size.scale, includeHidden, includePins },
      });
      await save.mutateAsync({ path, pngBase64 });
      toast.success("Map exported", { description: path });
      onClose();
    } catch (error) {
      toastError(error, "map-export");
    } finally {
      setBusy(false);
    }
  }

  return (
    <form onSubmit={(e) => void submit(e)} className="flex flex-col gap-4">
      <DialogHeader>
        <DialogTitle>Export map as PNG</DialogTitle>
        <DialogDescription>
          The whole map is drawn, not just the part you can see.
        </DialogDescription>
      </DialogHeader>

      <div className="flex flex-col gap-1.5">
        <Label htmlFor="export-scale" className="text-sm">
          Scale
        </Label>
        <select
          id="export-scale"
          value={scale}
          disabled={busy}
          onChange={(e) => setScale(Number(e.target.value))}
          className="h-8 rounded-md border bg-background px-2 text-sm"
        >
          {EXPORT_SCALES.map((s) => (
            <option key={s} value={s}>
              {s}× ({Math.floor(map.width * s)} × {Math.floor(map.height * s)} px)
            </option>
          ))}
        </select>
        <p className="text-xs text-muted-foreground" aria-live="polite">
          Image size: {size.width} × {size.height} px
          {size.clamped && " (reduced to the largest size allowed)"}
        </p>
      </div>

      <label className="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={includePins}
          disabled={busy}
          onChange={(e) => setIncludePins(e.target.checked)}
        />
        Include pins
      </label>
      <label className="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={includeHidden}
          disabled={busy || !hasHidden}
          onChange={(e) => setIncludeHidden(e.target.checked)}
        />
        Include hidden layers
        {!hasHidden && <span className="text-xs text-muted-foreground">(none hidden)</span>}
      </label>

      <DialogFooter>
        <Button type="button" variant="outline" disabled={busy} onClick={onClose}>
          Cancel
        </Button>
        <Button type="submit" disabled={busy}>
          {busy ? "Exporting…" : "Export…"}
        </Button>
      </DialogFooter>
    </form>
  );
}
