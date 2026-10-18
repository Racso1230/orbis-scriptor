import { useState } from "react";
import { Link2, Link2Off, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useCreateEntity } from "@/features/entities/queries";
import type { MapLayer, MapObject } from "@/lib/bindings";
import { EntityPickerDialog } from "./entity-picker-dialog";
import { displayOrder } from "./layers";
import { useMapActions } from "./map-actions-context";
import { useMapToolStore } from "./map-tool-store";
import { PROPERTY_EDITORS } from "./property-editors";
import { SelectControl } from "./style-controls";

const KIND_LABELS = {
  pin: "Pin",
  path: "Path",
  area: "Area",
  label: "Label",
  stamp: "Stamp",
} as const;

interface Props {
  object: MapObject;
  layers: MapLayer[];
}

/**
 * Properties of the selected object: kind-specific style fields (from the
 * `PROPERTY_EDITORS` registry), then the linked entity, the layer and delete.
 * Every change goes through the undo system.
 */
export function PropertiesPanel({ object, layers }: Props) {
  const actions = useMapActions();
  const select = useMapToolStore((s) => s.select);
  const createEntity = useCreateEntity();
  const [picking, setPicking] = useState(false);

  const kind = object.geometry.kind;
  const Editor = PROPERTY_EDITORS[kind];
  const layer = layers.find((l) => l.id === object.layerId);
  const locked = layer?.locked ?? false;

  function changeStyle(label: string, changes: { [key in string]: unknown }) {
    actions.update(label, [{ id: object.id, patch: { style: { ...object.style, ...changes } } }]);
  }

  function link(entityId: string) {
    setPicking(false);
    actions.update("Link entity", [{ id: object.id, patch: { entityId } }]);
  }

  async function createAndLink(type: string, name: string) {
    try {
      const entity = await createEntity.mutateAsync({ type, name });
      link(entity.id);
    } catch {
      setPicking(false); // the global toast already explained the failure
    }
  }

  const layerOptions = displayOrder(layers)
    .filter((l) => !l.locked || l.id === object.layerId)
    .map((l) => ({ value: l.id, label: l.name }));

  return (
    <section aria-label="Properties" className="flex flex-col gap-3">
      <h3 className="text-xs font-semibold tracking-wide text-muted-foreground uppercase">
        {KIND_LABELS[kind]} properties
      </h3>
      {locked && <p className="text-xs text-muted-foreground">This layer is locked.</p>}

      {Editor && <Editor object={object} disabled={locked} onStyleChange={changeStyle} />}

      <div className="flex flex-col gap-1">
        <span className="text-xs font-medium">Linked entity</span>
        <div className="flex items-center gap-1">
          <span className="min-w-0 flex-1 truncate text-xs text-muted-foreground">
            {object.entityId ? (object.entityName ?? "Linked") : "None"}
          </span>
          <Button
            variant="ghost"
            size="xs"
            disabled={locked}
            onClick={() => setPicking(true)}
            aria-label={object.entityId ? "Change linked entity" : "Link to entity"}
          >
            <Link2 /> {object.entityId ? "Change" : "Link"}
          </Button>
          {object.entityId && (
            <Button
              variant="ghost"
              size="icon-xs"
              disabled={locked}
              aria-label="Unlink entity"
              title="Unlink entity"
              onClick={() =>
                actions.update("Unlink entity", [{ id: object.id, patch: { unlinkEntity: true } }])
              }
            >
              <Link2Off />
            </Button>
          )}
        </div>
      </div>

      {layerOptions.length > 1 && (
        <SelectControl
          label="Layer"
          value={object.layerId}
          options={layerOptions}
          disabled={locked}
          onCommit={(layerId) =>
            actions.update("Move to layer", [{ id: object.id, patch: { layerId } }])
          }
        />
      )}

      <Button
        variant="destructive"
        size="sm"
        disabled={locked}
        onClick={() => {
          actions.remove(`Delete ${KIND_LABELS[kind].toLowerCase()}`, [object.id]);
          select(null);
        }}
      >
        <Trash2 /> Delete {KIND_LABELS[kind].toLowerCase()}
      </Button>

      <EntityPickerDialog
        open={picking}
        title="Link to an entity"
        description="Choose an existing entity or create a new one to link this object to"
        onOpenChange={setPicking}
        onPickEntity={link}
        onCreateEntity={(type, name) => void createAndLink(type, name)}
      />
    </section>
  );
}
