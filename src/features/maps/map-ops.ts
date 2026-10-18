import type {
  MapData,
  MapGeometry,
  MapObject,
  MapObjectPatch,
  MapOp,
  NewMapObject,
} from "@/lib/bindings";

/**
 * One user action, as a pair of op batches. `do` is sent to the backend when
 * the user acts (and again on redo); `undo` reverses it. Both are computed on
 * the client from the objects as they are *now*, so undo never needs the
 * server to remember anything.
 */
export interface MapAction {
  /** Short description, e.g. "Move pin" (shown as a tooltip on Undo/Redo). */
  label: string;
  do: MapOp[];
  undo: MapOp[];
}

export type StyleObject = { [key in string]: unknown };

// ---- ids and timestamps -------------------------------------------------

/** A fresh id for a client-created object. The backend accepts any UUID. */
export function newId(): string {
  return crypto.randomUUID();
}

let lastStamp = 0;
/**
 * ISO timestamp for `createdAt`. Strictly increasing (at least 1 ms apart), so
 * objects created in one gesture keep a stable draw order.
 */
export function nextTimestamp(now: number = Date.now()): string {
  lastStamp = Math.max(now, lastStamp + 1);
  return new Date(lastStamp).toISOString();
}

// ---- builders -----------------------------------------------------------

export interface NewObjectInput {
  layerId: string;
  geometry: MapGeometry;
  style?: StyleObject | null;
  entityId?: string | null;
}

/** Completes an input with a client id and creation time. */
export function toNewObject(input: NewObjectInput): NewMapObject {
  return {
    id: newId(),
    layerId: input.layerId,
    geometry: input.geometry,
    style: input.style ?? null,
    entityId: input.entityId ?? null,
    createdAt: nextTimestamp(),
  };
}

/** Create objects; undo deletes them again (in reverse order). */
export function buildCreate(label: string, objects: NewMapObject[]): MapAction {
  const undo: MapOp[] = [];
  for (const o of objects.slice().reverse()) {
    if (o.id) undo.push({ op: "delete", id: o.id });
  }
  return { label, do: objects.map((object) => ({ op: "create", object })), undo };
}

/** The patch that puts `current` back after `patch` was applied. */
export function inversePatch(current: MapObject, patch: MapObjectPatch): MapObjectPatch {
  const inverse: MapObjectPatch = { geometry: null, style: null, entityId: null };
  if (patch.geometry) inverse.geometry = current.geometry;
  if (patch.style) inverse.style = current.style;
  if (patch.layerId) inverse.layerId = current.layerId;
  if (patch.unlinkEntity || patch.entityId) {
    if (current.entityId) inverse.entityId = current.entityId;
    else inverse.unlinkEntity = true;
  }
  return inverse;
}

export interface UpdateInput {
  id: string;
  patch: Partial<MapObjectPatch>;
}

function fullPatch(patch: Partial<MapObjectPatch>): MapObjectPatch {
  return { geometry: null, style: null, entityId: null, ...patch };
}

/** Update objects; undo restores the previous values of the fields touched. */
export function buildUpdate(
  label: string,
  objects: readonly MapObject[],
  updates: UpdateInput[],
): MapAction | null {
  const byId = new Map(objects.map((o) => [o.id, o]));
  const doOps: MapOp[] = [];
  const undoOps: MapOp[] = [];
  for (const { id, patch } of updates) {
    const current = byId.get(id);
    if (!current) continue; // object vanished: nothing to update
    const full = fullPatch(patch);
    doOps.push({ op: "update", id, patch: full });
    undoOps.unshift({ op: "update", id, patch: inversePatch(current, full) });
  }
  return doOps.length === 0 ? null : { label, do: doOps, undo: undoOps };
}

function toNew(object: MapObject): NewMapObject {
  return {
    id: object.id,
    layerId: object.layerId,
    geometry: object.geometry,
    style: object.style,
    entityId: object.entityId,
    createdAt: object.createdAt,
  };
}

/**
 * Delete objects. Undo recreates them with their original id, creation time,
 * style and link, so stacking order is kept. Paths are recreated before
 * anything else, and any label that was following a deleted path (but is not
 * itself deleted) gets its `pathId` back with an extra update, because the
 * backend detaches such labels when their path goes away.
 */
export function buildDelete(
  label: string,
  objects: readonly MapObject[],
  ids: string[],
): MapAction | null {
  const doomed = new Set(ids);
  const victims = objects.filter((o) => doomed.has(o.id));
  if (victims.length === 0) return null;

  const rank = (o: MapObject) => (o.geometry.kind === "path" ? 0 : 1);
  const recreate = victims.slice().sort((a, b) => rank(a) - rank(b));

  const undo: MapOp[] = recreate.map((o) => ({ op: "create", object: toNew(o) }));
  for (const o of objects) {
    const g = o.geometry;
    if (doomed.has(o.id) || g.kind !== "label" || !g.pathId || !doomed.has(g.pathId)) continue;
    undo.push({ op: "update", id: o.id, patch: { geometry: g, style: null, entityId: null } });
  }

  return { label, do: victims.map((o) => ({ op: "delete", id: o.id })), undo };
}

// ---- optimistic application -------------------------------------------

/** Placeholder `MapObject` for an optimistic create (the server fills in the rest). */
function toObject(n: NewMapObject, now: string): MapObject {
  return {
    id: n.id ?? newId(),
    layerId: n.layerId,
    geometry: n.geometry,
    style: n.style ?? {},
    entityId: n.entityId,
    entityType: null,
    entityName: null,
    createdAt: n.createdAt ?? now,
    updatedAt: now,
  };
}

/**
 * Applies ops to cached map data the way the backend will (create / update /
 * delete in order, labels detached from deleted paths). Used for optimistic
 * updates so the canvas responds instantly.
 */
export function applyOpsToData(
  data: MapData,
  ops: MapOp[],
  now = new Date().toISOString(),
): MapData {
  let objects = data.objects;
  for (const op of ops) {
    if (op.op === "create") {
      objects = [...objects.filter((o) => o.id !== op.object.id), toObject(op.object, now)];
    } else if (op.op === "update") {
      objects = objects.map((o) => {
        if (o.id !== op.id) return o;
        const p = op.patch;
        const relinked = Boolean(p.unlinkEntity || p.entityId);
        return {
          ...o,
          geometry: p.geometry ?? o.geometry,
          style: p.style ?? o.style,
          layerId: p.layerId ?? o.layerId,
          entityId: p.unlinkEntity ? null : (p.entityId ?? o.entityId),
          // Names are filled in by the refetch that follows.
          entityName: relinked ? null : o.entityName,
          entityType: relinked ? null : o.entityType,
          updatedAt: now,
        };
      });
    } else {
      const id = op.id;
      objects = objects
        .filter((o) => o.id !== id)
        .map((o) =>
          o.geometry.kind === "label" && o.geometry.pathId === id
            ? { ...o, geometry: { ...o.geometry, pathId: null } }
            : o,
        );
    }
  }
  return { ...data, objects };
}
