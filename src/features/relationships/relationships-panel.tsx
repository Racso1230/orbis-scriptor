import { useMemo, useState } from "react";
import { Pencil, Plus, Trash2, Waypoints, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { EntityIcon } from "@/features/entities/entity-icon";
import { useCreateEntity, useEntities, useEntityTypes } from "@/features/entities/queries";
import { useOpenEntity } from "@/features/entities/use-open-entity";
import { EntityPickerDialog } from "@/features/maps/entity-picker-dialog";
import type { RelationshipKindDef, RelationshipView } from "@/lib/bindings";
import { useOpenGraph } from "./use-open-graph";
import {
  CATEGORY_LABELS,
  endpoints,
  groupByCategory,
  kindsForPair,
  offeredKinds,
  optionId,
} from "./kinds";
import {
  useCreateRelationship,
  useDeleteRelationship,
  useRelationshipKinds,
  useRelationships,
  useUpdateRelationship,
} from "./queries";

const SELECT_CLASS =
  "h-8 rounded-lg border border-input bg-background px-2 text-sm text-foreground outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50";

/** "Relationships" section of an entity page: list, edit, delete and add. */
export function RelationshipsPanel({
  entityId,
  entityType,
}: {
  entityId: string;
  entityType: string;
}) {
  const relationships = useRelationships(entityId);
  const kinds = useRelationshipKinds();
  const types = useEntityTypes();
  const openEntity = useOpenEntity();
  const openGraph = useOpenGraph();
  const deleteRelationship = useDeleteRelationship();
  const [adding, setAdding] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);

  const groups = useMemo(() => groupByCategory(relationships.data ?? []), [relationships.data]);

  return (
    <section aria-label="Relationships" className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-medium text-muted-foreground">
          Relationships{relationships.data ? ` (${relationships.data.length})` : ""}
        </h2>
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="sm" onClick={() => openGraph(entityId)}>
            <Waypoints /> Show in graph
          </Button>
          <Button variant="ghost" size="sm" onClick={() => setAdding((a) => !a)}>
            <Plus /> Add relationship
          </Button>
        </div>
      </div>

      {adding && (
        <AddRelationshipForm
          entityId={entityId}
          entityType={entityType}
          kinds={kinds.data ?? []}
          onDone={() => setAdding(false)}
        />
      )}

      {relationships.data?.length === 0 && !adding && (
        <p className="text-sm text-muted-foreground">
          No relationships yet. Add family, allies, memberships and more.
        </p>
      )}

      {groups.map((group) => (
        <div key={group.category} className="flex flex-col gap-1">
          <h3 className="px-3 pt-1 text-xs font-medium tracking-wide text-muted-foreground uppercase">
            {CATEGORY_LABELS[group.category]}
          </h3>
          <ul className="flex flex-col gap-1">
            {group.items.map((view) => {
              const type = types.data?.find((t) => t.key === view.otherType);
              return (
                <li key={view.linkId} className="flex flex-col gap-1">
                  <div className="group flex items-center gap-1">
                    <div className="flex min-w-0 flex-1 items-center gap-2 rounded-md px-3 py-2 text-sm">
                      <span className="shrink-0 text-muted-foreground">{view.label}</span>
                      <EntityIcon
                        icon={type?.icon ?? ""}
                        className="size-4 shrink-0 text-muted-foreground"
                      />
                      <button
                        type="button"
                        onClick={() => openEntity(view.otherId)}
                        className="truncate rounded-sm font-medium hover:underline"
                      >
                        {view.otherName}
                      </button>
                    </div>
                    <Button
                      variant="ghost"
                      size="icon-xs"
                      aria-label={`Edit relationship with ${view.otherName}`}
                      onClick={() => setEditingId(editingId === view.linkId ? null : view.linkId)}
                    >
                      <Pencil />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon-xs"
                      aria-label={`Delete relationship with ${view.otherName}`}
                      disabled={deleteRelationship.isPending}
                      onClick={() => deleteRelationship.mutate(view.linkId)}
                    >
                      <Trash2 />
                    </Button>
                  </div>
                  {editingId === view.linkId && (
                    <EditRelationshipForm
                      view={view}
                      entityType={entityType}
                      kinds={kinds.data ?? []}
                      onDone={() => setEditingId(null)}
                    />
                  )}
                </li>
              );
            })}
          </ul>
        </div>
      ))}
      {deleteRelationship.error && (
        <p className="text-sm text-destructive">{deleteRelationship.error.message}</p>
      )}
    </section>
  );
}

/** Pick a kind (worded from this entity's side), then the other entity. */
function AddRelationshipForm({
  entityId,
  entityType,
  kinds,
  onDone,
}: {
  entityId: string;
  entityType: string;
  kinds: RelationshipKindDef[];
  onDone: () => void;
}) {
  const create = useCreateRelationship();
  const createEntity = useCreateEntity();
  const entities = useEntities();
  const [optionKey, setOptionKey] = useState("");
  const [otherId, setOtherId] = useState<string | null>(null);
  const [label, setLabel] = useState("");
  const [picking, setPicking] = useState(false);

  const options = useMemo(() => offeredKinds(kinds, entityType), [kinds, entityType]);
  const groups = groupByCategory(options.map((o) => ({ ...o, category: o.def.category })));
  const option = options.find((o) => optionId(o) === optionKey);
  const otherName = entities.data?.find((e) => e.id === otherId)?.name;
  const labelOk = !option?.def.needsLabel || label.trim().length > 0;

  function changeKind(next: string) {
    setOptionKey(next);
    // The new kind may not allow the entity picked before.
    const nextOption = options.find((o) => optionId(o) === next);
    const other = entities.data?.find((e) => e.id === otherId);
    if (
      other &&
      nextOption &&
      nextOption.otherTypes.length > 0 &&
      !nextOption.otherTypes.includes(other.type)
    ) {
      setOtherId(null);
    }
  }

  async function handleCreateEntity(type: string, name: string) {
    setPicking(false);
    const entity = await createEntity.mutateAsync({ type, name });
    setOtherId(entity.id);
  }

  async function submit() {
    if (!option || !otherId) return;
    await create.mutateAsync({
      ...endpoints(option, entityId, otherId),
      kind: option.def.key,
      label: option.def.needsLabel ? label.trim() : null,
    });
    onDone();
  }

  return (
    <div className="flex flex-col gap-3 rounded-lg border p-3">
      <div className="flex flex-wrap items-center gap-2">
        <select
          aria-label="Relationship"
          className={SELECT_CLASS}
          value={optionKey}
          onChange={(e) => changeKind(e.target.value)}
        >
          <option value="" disabled>
            This {entityType} is…
          </option>
          {groups.map((group) => (
            <optgroup key={group.category} label={CATEGORY_LABELS[group.category]}>
              {group.items.map((o) => (
                <option key={optionId(o)} value={optionId(o)}>
                  {o.label}
                </option>
              ))}
            </optgroup>
          ))}
        </select>
        <Button
          variant="outline"
          size="sm"
          disabled={!option}
          onClick={() => setPicking(true)}
          aria-label="Choose entity"
        >
          {otherName ?? "Choose entity…"}
        </Button>
        {option?.def.needsLabel && (
          <Input
            aria-label="Custom label"
            placeholder="e.g. covets"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            className="w-40"
          />
        )}
      </div>
      <div className="flex items-center gap-2">
        <Button
          size="sm"
          disabled={!option || !otherId || !labelOk || create.isPending}
          onClick={() => void submit().catch(() => undefined)}
        >
          Add
        </Button>
        <Button variant="ghost" size="sm" onClick={onDone}>
          <X /> Cancel
        </Button>
      </div>
      {create.error && <p className="text-sm text-destructive">{create.error.message}</p>}
      <EntityPickerDialog
        open={picking}
        onOpenChange={setPicking}
        onPickEntity={(id) => {
          setPicking(false);
          setOtherId(id);
        }}
        onCreateEntity={(type, name) => void handleCreateEntity(type, name)}
        title="Choose the other entity"
        description={option ? `This ${entityType} is ${option.label}…` : "Choose an entity"}
        allowedTypes={option?.otherTypes}
        excludeId={entityId}
      />
    </div>
  );
}

/** Change kind / label / direction of an existing relationship. */
function EditRelationshipForm({
  view,
  entityType,
  kinds,
  onDone,
}: {
  view: RelationshipView;
  entityType: string;
  kinds: RelationshipKindDef[];
  onDone: () => void;
}) {
  const update = useUpdateRelationship();
  const [kind, setKind] = useState(view.kind);
  const [label, setLabel] = useState(view.kind === "custom" ? view.label : "");
  const [swap, setSwap] = useState(false);

  const [sourceType, targetType] = view.outgoing
    ? [entityType, view.otherType]
    : [view.otherType, entityType];
  const valid = kindsForPair(kinds, sourceType, targetType);
  const chosen = kinds.find((k) => k.key === kind);
  const wording = (k: RelationshipKindDef) => (view.outgoing ? k.forwardLabel : k.inverseLabel);

  async function save() {
    await update.mutateAsync({
      linkId: view.linkId,
      kind: kind !== view.kind ? kind : null,
      label: chosen?.needsLabel ? label.trim() : null,
      swapDirection: swap ? true : null,
    });
    onDone();
  }

  return (
    <div className="ml-3 flex flex-col gap-2 rounded-lg border p-3">
      <div className="flex flex-wrap items-center gap-2">
        <select
          aria-label="Kind"
          className={SELECT_CLASS}
          value={kind}
          onChange={(e) => setKind(e.target.value)}
        >
          {valid.map((k) => (
            <option key={k.key} value={k.key}>
              {wording(k)}
            </option>
          ))}
        </select>
        {chosen?.needsLabel && (
          <Input
            aria-label="Custom label"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            className="w-40"
          />
        )}
        {!view.symmetric && (
          <label className="flex items-center gap-2 text-sm">
            <input type="checkbox" checked={swap} onChange={(e) => setSwap(e.target.checked)} />
            Reverse direction
          </label>
        )}
      </div>
      <div className="flex items-center gap-2">
        <Button
          size="sm"
          disabled={update.isPending || (chosen?.needsLabel === true && label.trim() === "")}
          onClick={() => void save().catch(() => undefined)}
        >
          Save
        </Button>
        <Button variant="ghost" size="sm" onClick={onDone}>
          <X /> Cancel
        </Button>
      </div>
      {update.error && <p className="text-sm text-destructive">{update.error.message}</p>}
    </div>
  );
}
