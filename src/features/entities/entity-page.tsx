import { useCallback, useState } from "react";
import { useNavigate, useParams } from "@tanstack/react-router";
import { Trash2 } from "lucide-react";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { LoreEditor } from "@/features/lore/lore-editor";
import { useCurrentWorld } from "@/features/worlds/queries";
import type { Entity, EntityTypeDef, FieldDef } from "@/lib/bindings";
import { useTabsStore } from "@/stores/tabs-store";
import { EntityIcon } from "./entity-icon";
import { useDeleteEntity, useEntity, useEntityTypes, useUpdateEntity } from "./queries";
import { SaveIndicator } from "./save-indicator";
import { useAutosave, type SaveStatus } from "./use-autosave";

type Properties = Entity["properties"];

/** The fields form and the lore editor save separately; show the "worst" state. */
const STATUS_PRIORITY: SaveStatus[] = ["error", "saving", "dirty", "saved"];
function combineStatus(a: SaveStatus, b: SaveStatus): SaveStatus {
  return STATUS_PRIORITY.find((s) => s === a || s === b) ?? "saved";
}

/** Route component for `/world/entity/$id`. */
export function EntityPage() {
  const { id } = useParams({ from: "/world/entity/$id" });
  const entity = useEntity(id);
  const types = useEntityTypes();

  if (entity.isPending || types.isPending) {
    return <p className="p-6 text-sm text-muted-foreground">Loading…</p>;
  }
  if (entity.error || !entity.data) {
    return (
      <p className="p-6 text-sm text-destructive">
        {entity.error?.message ?? "This entity no longer exists."}
      </p>
    );
  }
  const type = types.data?.find((t) => t.key === entity.data.type);
  // `key` remounts the form when switching entities so local state resets.
  return <EntityForm key={entity.data.id} entity={entity.data} type={type} />;
}

function EntityForm({ entity, type }: { entity: Entity; type: EntityTypeDef | undefined }) {
  const navigate = useNavigate();
  const world = useCurrentWorld();
  const updateEntity = useUpdateEntity();
  const deleteEntity = useDeleteEntity();
  const closeTab = useTabsStore((s) => s.closeTab);

  const [name, setName] = useState(entity.name);
  const [summary, setSummary] = useState(entity.summary);
  const [properties, setProperties] = useState<Properties>(entity.properties);

  const draft = { name, summary, properties };
  const fields = useAutosave(draft, (value) =>
    updateEntity.mutateAsync({ id: entity.id, patch: value }),
  );
  const [lore, setLore] = useState<{ status: SaveStatus; error: string | null }>({
    status: "saved",
    error: null,
  });
  // Stable callback so the editor's effect doesn't re-run every render.
  const onLoreStatus = useCallback(
    (status: SaveStatus, error: string | null) => setLore({ status, error }),
    [],
  );
  const status = combineStatus(fields.status, lore.status);
  const error = fields.error ?? lore.error;

  function setProperty(key: string, value: unknown) {
    setProperties((prev) => {
      const next = { ...prev };
      if (value === undefined || value === "") delete next[key];
      else next[key] = value;
      return next;
    });
  }

  async function handleDelete() {
    await deleteEntity.mutateAsync(entity.id);
    const next = world.data ? closeTab(world.data.path, entity.id) : null;
    if (next) await navigate({ to: "/world/entity/$id", params: { id: next } });
    else await navigate({ to: "/world" });
  }

  return (
    <div className="flex w-full max-w-3xl flex-col gap-6 p-6">
      <header className="flex items-center gap-3">
        <EntityIcon icon={type?.icon ?? ""} className="size-6 shrink-0 text-muted-foreground" />
        <Input
          value={name}
          onChange={(e) => setName(e.target.value)}
          aria-label="Name"
          className="h-auto border-none px-1 text-2xl font-semibold shadow-none focus-visible:ring-0"
        />
        <Badge variant="secondary">{type?.label ?? entity.type}</Badge>
        <SaveIndicator status={status} error={error} />
        <AlertDialog>
          <AlertDialogTrigger asChild>
            <Button variant="ghost" size="icon" aria-label="Delete entity">
              <Trash2 />
            </Button>
          </AlertDialogTrigger>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>Delete “{name || entity.name}”?</AlertDialogTitle>
              <AlertDialogDescription>
                This removes the {type?.label.toLowerCase() ?? "entity"} and cannot be undone.
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>Cancel</AlertDialogCancel>
              <AlertDialogAction onClick={handleDelete}>Delete</AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </header>

      <div className="flex flex-col gap-2">
        <Label htmlFor="summary">Summary</Label>
        <Textarea
          id="summary"
          value={summary}
          onChange={(e) => setSummary(e.target.value)}
          placeholder="One or two sentences…"
          rows={3}
        />
      </div>

      {type && type.fields.length > 0 && (
        <div className="grid grid-cols-2 gap-4">
          {type.fields.map((field) => (
            <FieldInput
              key={field.key}
              field={field}
              value={properties[field.key]}
              onChange={(value) => setProperty(field.key, value)}
            />
          ))}
        </div>
      )}

      <div className="flex flex-col gap-2">
        <Label>Lore</Label>
        <LoreEditor entityId={entity.id} onStatusChange={onLoreStatus} />
      </div>
    </div>
  );
}

function FieldInput({
  field,
  value,
  onChange,
}: {
  field: FieldDef;
  value: unknown;
  onChange: (value: unknown) => void;
}) {
  const inputId = `field-${field.key}`;
  return (
    <div className="flex flex-col gap-2">
      <Label htmlFor={inputId}>{field.label}</Label>
      {field.kind === "select" ? (
        <Select value={typeof value === "string" ? value : ""} onValueChange={(v) => onChange(v)}>
          <SelectTrigger id={inputId} className="w-full">
            <SelectValue placeholder="—" />
          </SelectTrigger>
          <SelectContent>
            {field.options.map((option) => (
              <SelectItem key={option} value={option}>
                {option}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      ) : field.kind === "number" ? (
        <Input
          id={inputId}
          type="number"
          value={typeof value === "number" ? value : ""}
          onChange={(e) => onChange(e.target.value === "" ? undefined : Number(e.target.value))}
        />
      ) : (
        <Input
          id={inputId}
          value={typeof value === "string" ? value : ""}
          onChange={(e) => onChange(e.target.value)}
        />
      )}
    </div>
  );
}
