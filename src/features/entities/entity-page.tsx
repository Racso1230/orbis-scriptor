import { useCallback, useState } from "react";
import { useNavigate, useParams } from "@tanstack/react-router";
import { Printer, Star, Trash2 } from "lucide-react";
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
import { Textarea } from "@/components/ui/textarea";
import { BacklinksPanel } from "@/features/lore/backlinks-panel";
import { LoreEditor } from "@/features/lore/lore-editor";
import { AppearsOnMaps } from "@/features/maps/appears-on-maps";
import { GallerySection } from "@/features/gallery/gallery-section";
import { Lightbox } from "@/features/gallery/lightbox";
import { useEntityImages } from "@/features/gallery/queries";
import { MapView } from "@/features/maps/map-view";
import { EntityTags } from "@/features/tags/entity-tags";
import { EntityEvents } from "@/features/timeline/entity-events";
import { EventDates } from "@/features/timeline/event-dates";
import { InvolvedPanel } from "@/features/timeline/involved-panel";
import { RelationshipsPanel } from "@/features/relationships/relationships-panel";
import { useCurrentWorld } from "@/features/worlds/queries";
import type { Entity, EntityTypeDef } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { useTabsStore } from "@/stores/tabs-store";
import { EntityAvatar } from "./entity-avatar";
import { FieldInput } from "./field-inputs";
import { isUnset } from "./field-values";
import {
  useDeleteEntity,
  useEntity,
  useEntityTypes,
  useSetFavorite,
  useUpdateEntity,
} from "./queries";
import { SaveIndicator } from "./save-indicator";
import { useSettingsValue } from "@/features/settings/queries";
import { useAutosave, type SaveStatus } from "./use-autosave";

type Properties = Entity["properties"];

/** The fields form and the lore editor save separately; show the "worst" state. */
const STATUS_PRIORITY: SaveStatus[] = ["error", "saving", "dirty", "saved"];
function combineStatus(a: SaveStatus, b: SaveStatus): SaveStatus {
  return STATUS_PRIORITY.find((s) => s === a || s === b) ?? "saved";
}

/** Route component for `/world/entity/$id`. */
export function EntityPage() {
  // "/shell" is the pathless layout route that holds the app header (see router.tsx).
  const { id } = useParams({ from: "/shell/world/entity/$id" });
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
  const setFavorite = useSetFavorite();
  const gallery = useEntityImages(entity.id);
  const images = gallery.data ?? [];
  const [lightbox, setLightbox] = useState<number | null>(null);
  const closeTab = useTabsStore((s) => s.closeTab);

  const [name, setName] = useState(entity.name);
  const [summary, setSummary] = useState(entity.summary);
  const [properties, setProperties] = useState<Properties>(entity.properties);

  // Built-in fields first, then the ones the user added to the type.
  const builtinFields = (type?.fields ?? []).filter((f) => !f.custom);
  const customFields = (type?.fields ?? []).filter((f) => f.custom);

  const isMap = entity.type === "map";
  const isEvent = entity.type === "event";

  const draft = { name, summary, properties };
  const { autosaveDelayMs } = useSettingsValue();
  const fields = useAutosave(
    draft,
    (value) => updateEntity.mutateAsync({ id: entity.id, patch: value }),
    autosaveDelayMs,
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
  const [eventSave, setEventSave] = useState<{ status: SaveStatus; error: string | null }>({
    status: "saved",
    error: null,
  });
  const onEventStatus = useCallback(
    (status: SaveStatus, error: string | null) => setEventSave({ status, error }),
    [],
  );
  const status = combineStatus(combineStatus(fields.status, lore.status), eventSave.status);
  const error = fields.error ?? lore.error ?? eventSave.error;

  function setProperty(key: string, value: unknown) {
    setProperties((prev) => {
      const next = { ...prev };
      if (isUnset(value)) delete next[key];
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
    <div
      className={cn(
        "flex w-full flex-col gap-6 p-6",
        // Maps get the whole width; other entities read better in a narrow column.
        isMap ? "max-w-none" : "max-w-3xl",
      )}
    >
      <header className="flex items-center gap-3">
        {entity.portrait ? (
          <button
            type="button"
            aria-label="Open portrait"
            className="shrink-0 rounded-full"
            onClick={() => {
              const index = images.findIndex((i) => i.isPortrait);
              if (index >= 0) setLightbox(index);
            }}
          >
            <EntityAvatar type={type} portrait={entity.portrait} className="size-14" />
          </button>
        ) : (
          <EntityAvatar type={type} portrait={null} className="size-6 opacity-70" />
        )}
        <Input
          value={name}
          onChange={(e) => setName(e.target.value)}
          aria-label="Name"
          className="h-auto border-none px-1 text-2xl font-semibold shadow-none focus-visible:ring-0"
        />
        <Badge variant="secondary">{type?.label ?? entity.type}</Badge>
        <SaveIndicator status={status} error={error} />
        <Button
          variant="ghost"
          size="icon"
          aria-label={entity.favorite ? "Remove from favourites" : "Add to favourites"}
          aria-pressed={entity.favorite}
          title={entity.favorite ? "Remove from favourites" : "Add to favourites"}
          disabled={setFavorite.isPending}
          onClick={() => setFavorite.mutate({ id: entity.id, favorite: !entity.favorite })}
        >
          <Star className={cn(entity.favorite && "fill-amber-400 text-amber-500")} />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Print or save as PDF"
          title="Print / Save as PDF…"
          onClick={() => void navigate({ to: "/print/entity/$id", params: { id: entity.id } })}
        >
          <Printer />
        </Button>
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

      <EntityTags entityId={entity.id} tags={entity.tags} />

      {isMap && <MapView mapEntityId={entity.id} />}

      {isEvent && <EventDates entityId={entity.id} onStatusChange={onEventStatus} />}

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
        <>
          <div className="grid grid-cols-2 gap-4">
            {builtinFields.map((field) => (
              <FieldInput
                key={field.key}
                field={field}
                value={properties[field.key]}
                entityId={entity.id}
                error={fields.error}
                onChange={(value) => setProperty(field.key, value)}
              />
            ))}
          </div>
          {customFields.length > 0 && (
            <div className="flex flex-col gap-3">
              {builtinFields.length > 0 && (
                <h2 className="text-sm font-medium text-muted-foreground">Extra fields</h2>
              )}
              <div className="grid grid-cols-2 gap-4">
                {customFields.map((field) => (
                  <FieldInput
                    key={field.key}
                    field={field}
                    value={properties[field.key]}
                    entityId={entity.id}
                    error={fields.error}
                    onChange={(value) => setProperty(field.key, value)}
                  />
                ))}
              </div>
            </div>
          )}
        </>
      )}

      <div className="flex flex-col gap-2">
        <Label>Lore</Label>
        <LoreEditor entityId={entity.id} onStatusChange={onLoreStatus} />
      </div>

      <GallerySection
        entityId={entity.id}
        images={images}
        refetch={gallery.refetch}
        onOpen={setLightbox}
      />

      {isEvent && <InvolvedPanel eventId={entity.id} />}

      {!isMap && <RelationshipsPanel entityId={entity.id} entityType={entity.type} />}

      <BacklinksPanel entityId={entity.id} />
      {!isMap && <AppearsOnMaps entityId={entity.id} />}
      {!isMap && !isEvent && <EntityEvents entityId={entity.id} />}
      <Lightbox
        images={images}
        index={lightbox}
        onIndexChange={setLightbox}
        onClose={() => setLightbox(null)}
      />
    </div>
  );
}
