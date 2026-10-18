import { useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  Eye,
  EyeOff,
  Lock,
  LockOpen,
  Pencil,
  Plus,
  Trash2,
} from "lucide-react";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { MapLayer } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import {
  useCreateLayer,
  useDeleteLayer,
  useRenameLayer,
  useReorderLayers,
  useSetLayerLocked,
  useSetLayerVisibility,
} from "./layer-queries";
import { deleteBlockedReason, displayOrder, nextLayerName, reorderedIds } from "./layers";

interface Props {
  mapEntityId: string;
  layers: MapLayer[];
  /** The layer new objects go to (already resolved to an editable layer, or null). */
  activeLayerId: string | null;
  onActivate: (layerId: string) => void;
}

/**
 * Lists the map's layers top first. Per layer: show/hide, lock, rename,
 * move up/down, delete. Clicking a layer's name makes it the active layer
 * (where new objects go). Layer operations are not undoable.
 */
export function LayersPanel({ mapEntityId, layers, activeLayerId, onActivate }: Props) {
  const create = useCreateLayer(mapEntityId);
  const rename = useRenameLayer(mapEntityId);
  const setVisible = useSetLayerVisibility(mapEntityId);
  const setLocked = useSetLayerLocked(mapEntityId);
  const reorder = useReorderLayers(mapEntityId);
  const remove = useDeleteLayer(mapEntityId);

  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [confirmDelete, setConfirmDelete] = useState<MapLayer | null>(null);

  const rows = displayOrder(layers);
  const error =
    create.error ??
    rename.error ??
    setVisible.error ??
    setLocked.error ??
    reorder.error ??
    remove.error;

  function startRename(layer: MapLayer) {
    setRenamingId(layer.id);
    setDraft(layer.name);
  }

  function finishRename(layer: MapLayer, commit: boolean) {
    setRenamingId(null);
    const name = draft.trim();
    if (commit && name !== "" && name !== layer.name) {
      rename.mutate({ layerId: layer.id, name });
    }
  }

  function move(layer: MapLayer, direction: "up" | "down") {
    const ids = reorderedIds(layers, layer.id, direction);
    if (ids) reorder.mutate(ids);
  }

  return (
    <section aria-label="Layers" className="flex flex-col gap-1.5">
      <div className="flex items-center justify-between">
        <h3 className="text-xs font-semibold tracking-wide text-muted-foreground uppercase">
          Layers
        </h3>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label="Add layer"
          title="Add layer"
          disabled={create.isPending}
          onClick={() =>
            create.mutate(nextLayerName(layers), { onSuccess: (l) => onActivate(l.id) })
          }
        >
          <Plus />
        </Button>
      </div>

      <ul className="flex flex-col gap-0.5">
        {rows.map((layer, index) => {
          const blocked = deleteBlockedReason(layer, layers);
          const active = layer.id === activeLayerId;
          return (
            <li
              key={layer.id}
              aria-current={active ? "true" : undefined}
              className={cn(
                "flex items-center gap-0.5 rounded-md border border-transparent px-1 py-0.5",
                active && "border-ring/60 bg-muted",
                !layer.visible && "opacity-60",
              )}
            >
              <Button
                variant="ghost"
                size="icon-xs"
                aria-label={`${layer.visible ? "Hide" : "Show"} layer ${layer.name}`}
                aria-pressed={layer.visible}
                title={layer.visible ? "Hide layer" : "Show layer"}
                onClick={() => setVisible.mutate({ layerId: layer.id, value: !layer.visible })}
              >
                {layer.visible ? <Eye /> : <EyeOff />}
              </Button>
              <Button
                variant="ghost"
                size="icon-xs"
                aria-label={`${layer.locked ? "Unlock" : "Lock"} layer ${layer.name}`}
                aria-pressed={layer.locked}
                title={layer.locked ? "Unlock layer" : "Lock layer"}
                onClick={() => setLocked.mutate({ layerId: layer.id, value: !layer.locked })}
              >
                {layer.locked ? <Lock /> : <LockOpen />}
              </Button>

              {renamingId === layer.id ? (
                <Input
                  autoFocus
                  aria-label={`Name of layer ${layer.name}`}
                  className="h-6 min-w-0 flex-1 px-1.5 text-xs"
                  value={draft}
                  maxLength={60}
                  onChange={(e) => setDraft(e.target.value)}
                  onBlur={() => finishRename(layer, true)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") finishRename(layer, true);
                    else if (e.key === "Escape") finishRename(layer, false);
                    e.stopPropagation();
                  }}
                />
              ) : (
                <button
                  type="button"
                  className="min-w-0 flex-1 truncate px-1 text-left text-xs"
                  title={
                    active
                      ? "Active layer: new objects go here"
                      : "Click to make this the active layer"
                  }
                  onClick={() => onActivate(layer.id)}
                  onDoubleClick={() => startRename(layer)}
                >
                  {layer.name}
                </button>
              )}

              <Button
                variant="ghost"
                size="icon-xs"
                aria-label={`Rename layer ${layer.name}`}
                title="Rename"
                onClick={() => startRename(layer)}
              >
                <Pencil />
              </Button>
              <Button
                variant="ghost"
                size="icon-xs"
                aria-label={`Move layer ${layer.name} up`}
                title="Move up"
                disabled={index === 0 || reorder.isPending}
                onClick={() => move(layer, "up")}
              >
                <ArrowUp />
              </Button>
              <Button
                variant="ghost"
                size="icon-xs"
                aria-label={`Move layer ${layer.name} down`}
                title="Move down"
                disabled={index === rows.length - 1 || reorder.isPending}
                onClick={() => move(layer, "down")}
              >
                <ArrowDown />
              </Button>
              <Button
                variant="ghost"
                size="icon-xs"
                aria-label={`Delete layer ${layer.name}`}
                title={blocked ?? "Delete layer"}
                disabled={blocked !== null}
                onClick={() => setConfirmDelete(layer)}
              >
                <Trash2 />
              </Button>
            </li>
          );
        })}
      </ul>

      {error && (
        <p role="alert" className="text-xs text-destructive">
          {error.message}
        </p>
      )}

      <AlertDialog
        open={confirmDelete !== null}
        onOpenChange={(open) => {
          if (!open) setConfirmDelete(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete layer “{confirmDelete?.name}”?</AlertDialogTitle>
            <AlertDialogDescription>
              Everything drawn on this layer is deleted with it. Deleting a layer cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                if (confirmDelete) remove.mutate(confirmDelete.id);
                setConfirmDelete(null);
              }}
            >
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </section>
  );
}
