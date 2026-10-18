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
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { validateBlankMap, type BlankMapForm } from "./blank-map";
import { BlankMapFields, type BlankMapFieldErrors } from "./blank-map-fields";
import { useConfigureBlankMap, useCreateBlankMap } from "./queries";

const INITIAL: BlankMapForm = {
  size: "small",
  customWidth: "3000",
  customHeight: "2000",
  background: { kind: "parchment", color: null },
};

type Target =
  /** Create a brand-new blank map entity (Ctrl+N, palette). */
  | { mode: "create"; onCreated: (entityId: string) => void }
  /** Turn an existing empty map into a blank one (the map's empty state). */
  | { mode: "configure"; mapEntityId: string };

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  target: Target;
}

/** "Blank map" dialog: name (new maps only), size preset or custom, background. */
export function BlankMapDialog({ open, onOpenChange, target }: Props) {
  const [form, setForm] = useState<BlankMapForm>(INITIAL);
  const [name, setName] = useState("New Map");
  const [errors, setErrors] = useState<BlankMapFieldErrors>({});
  const [failure, setFailure] = useState<string | null>(null);

  const createBlank = useCreateBlankMap();
  const configure = useConfigureBlankMap(target.mode === "configure" ? target.mapEntityId : "");
  const pending = createBlank.isPending || configure.isPending;

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
      if (target.mode === "create") {
        const trimmed = name.trim();
        if (!trimmed) {
          setFailure("Give the map a name.");
          return;
        }
        const data = await createBlank.mutateAsync({ name: trimmed, width, height, background });
        onOpenChange(false);
        target.onCreated(data.map.entityId);
      } else {
        await configure.mutateAsync({ size: { width, height }, background });
        onOpenChange(false);
      }
    } catch (err) {
      setFailure(err instanceof Error ? err.message : "Could not create the blank map.");
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <form onSubmit={(e) => void submit(e)} className="flex flex-col gap-4">
          <DialogHeader>
            <DialogTitle>Blank map</DialogTitle>
            <DialogDescription>
              Start from an empty canvas and draw the world yourself.
            </DialogDescription>
          </DialogHeader>

          {target.mode === "create" && (
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="blank-map-name">Name</Label>
              <Input
                id="blank-map-name"
                value={name}
                maxLength={120}
                onChange={(e) => setName(e.target.value)}
              />
            </div>
          )}

          <BlankMapFields value={form} onChange={setForm} errors={errors} />

          {failure && (
            <p role="alert" className="text-sm text-destructive">
              {failure}
            </p>
          )}
          <DialogFooter>
            <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={pending}>
              {pending ? "Creating…" : "Create"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
