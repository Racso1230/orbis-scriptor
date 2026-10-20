import { ImagePlus, Mountain } from "lucide-react";
import { Button } from "@/components/ui/button";
import { openGenerateWizard } from "@/features/generator/generate-store";

/**
 * Shown instead of the canvas while a map has no background: import an
 * image, start blank, or generate terrain into this very map.
 */
export function MapEmptyState(props: {
  mapEntityId: string;
  importing: boolean;
  onImport: () => void;
  onStartBlank: () => void;
}) {
  return (
    <div className="flex size-full flex-col items-center justify-center gap-3 text-muted-foreground">
      <p className="text-sm">This map has no background yet.</p>
      <div className="flex flex-wrap justify-center gap-2">
        <Button onClick={props.onImport} disabled={props.importing}>
          <ImagePlus /> {props.importing ? "Importing…" : "Import map image"}
        </Button>
        <Button variant="outline" onClick={props.onStartBlank}>
          Start blank
        </Button>
        <Button
          id="map-generate-terrain"
          variant="outline"
          onClick={() => openGenerateWizard({ targetMapEntityId: props.mapEntityId })}
        >
          <Mountain /> Generate terrain…
        </Button>
      </div>
    </div>
  );
}
