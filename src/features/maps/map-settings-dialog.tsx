import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { MapInfo } from "@/lib/bindings";
import { SIZE_PRESETS, validateBlankMap, type BlankMapForm } from "./blank-map";
import { BlankMapFields, type BlankMapFieldErrors } from "./blank-map-fields";
import { useConfigureBlankMap } from "./queries";

function initialForm(map: MapInfo): BlankMapForm {
  const preset = SIZE_PRESETS.find((p) => p.width === map.width && p.height === map.height);
  return {
    size: preset?.id ?? "custom",
    customWidth: String(map.width),
    customHeight: String(map.height),
    background: map.backgroundStyle ?? { kind: "parchment", color: null },
  };
}

interface Props {
  map: MapInfo;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** Map settings for a blank map: change the background look or resize the canvas. */
export function MapSettingsDialog({ map, open, onOpenChange }: Props) {
  // Remount the body each time the dialog opens, so it starts from the saved values.
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <SettingsForm map={map} onClose={() => onOpenChange(false)} />
      </DialogContent>
    </Dialog>
  );
}

function SettingsForm({ map, onClose }: { map: MapInfo; onClose: () => void }) {
  const [form, setForm] = useState<BlankMapForm>(() => initialForm(map));
  const [errors, setErrors] = useState<BlankMapFieldErrors>({});
  const [failure, setFailure] = useState<string | null>(null);
  const configure = useConfigureBlankMap(map.entityId);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setFailure(null);
    const result = validateBlankMap(form);
    if (!result.ok) {
      setErrors(result.errors);
      return;
    }
    setErrors({});
    const { width, height, background } = result.value;
    try {
      await configure.mutateAsync({
        // Objects outside a smaller canvas are kept; they just fall off the edge.
        ...(width !== map.width || height !== map.height ? { size: { width, height } } : {}),
        background,
      });
      onClose();
    } catch (err) {
      setFailure(err instanceof Error ? err.message : "Could not save the map settings.");
    }
  }

  return (
    <form onSubmit={(e) => void submit(e)} className="flex flex-col gap-4">
      <DialogHeader>
        <DialogTitle>Map settings</DialogTitle>
        <DialogDescription>
          Change the background or resize the canvas. Shrinking keeps your drawings; anything
          outside the new edge is just hidden.
        </DialogDescription>
      </DialogHeader>
      <BlankMapFields value={form} onChange={setForm} errors={errors} />
      {failure && (
        <p role="alert" className="text-sm text-destructive">
          {failure}
        </p>
      )}
      <DialogFooter>
        <Button type="button" variant="ghost" onClick={onClose}>
          Cancel
        </Button>
        <Button type="submit" disabled={configure.isPending}>
          Save
        </Button>
      </DialogFooter>
    </form>
  );
}
