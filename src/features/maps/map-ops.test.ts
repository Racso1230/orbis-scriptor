import { describe, expect, it } from "vitest";
import type { MapData, MapGeometry, MapObject } from "@/lib/bindings";
import {
  applyOpsToData,
  buildCreate,
  buildDelete,
  buildUpdate,
  inversePatch,
  nextTimestamp,
  toNewObject,
} from "./map-ops";

function obj(id: string, geometry: MapGeometry, over: Partial<MapObject> = {}): MapObject {
  return {
    id,
    layerId: "L1",
    geometry,
    style: {},
    entityId: null,
    entityType: null,
    entityName: null,
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
    ...over,
  };
}

const pin = (id: string, x = 1, y = 2, over: Partial<MapObject> = {}) =>
  obj(id, { kind: "pin", x, y }, over);

describe("buildCreate", () => {
  it("undoes by deleting the same ids in reverse order", () => {
    const a = toNewObject({ layerId: "L1", geometry: { kind: "pin", x: 0, y: 0 } });
    const b = toNewObject({ layerId: "L1", geometry: { kind: "pin", x: 1, y: 1 } });
    const action = buildCreate("Add", [a, b]);
    expect(action.do.map((o) => o.op)).toEqual(["create", "create"]);
    expect(action.undo).toEqual([
      { op: "delete", id: b.id },
      { op: "delete", id: a.id },
    ]);
  });

  it("gives every new object an id and strictly increasing createdAt", () => {
    const a = toNewObject({ layerId: "L", geometry: { kind: "pin", x: 0, y: 0 } });
    const b = toNewObject({ layerId: "L", geometry: { kind: "pin", x: 0, y: 0 } });
    expect(a.id).toBeTruthy();
    expect(a.id).not.toBe(b.id);
    expect(Date.parse(b.createdAt ?? "")).toBeGreaterThan(Date.parse(a.createdAt ?? ""));
    // Even when the clock does not move.
    const t1 = nextTimestamp(1000);
    const t2 = nextTimestamp(1000);
    expect(Date.parse(t2)).toBeGreaterThan(Date.parse(t1));
  });
});

describe("buildUpdate", () => {
  it("restores the previous geometry on undo", () => {
    const p = pin("p", 5, 6);
    const action = buildUpdate(
      "Move",
      [p],
      [{ id: "p", patch: { geometry: { kind: "pin", x: 9, y: 9 } } }],
    );
    expect(action?.do[0]).toMatchObject({
      op: "update",
      patch: { geometry: { x: 9, y: 9 }, style: null },
    });
    expect(action?.undo[0]).toMatchObject({
      op: "update",
      id: "p",
      patch: { geometry: { kind: "pin", x: 5, y: 6 }, style: null, entityId: null },
    });
  });

  it("restores style and layer", () => {
    const o = obj(
      "a",
      { kind: "area", points: [] as never },
      { style: { fill: "#fff" }, layerId: "L1" },
    );
    const inv = inversePatch(o, {
      geometry: null,
      style: { fill: "#000" },
      entityId: null,
      layerId: "L2",
    });
    expect(inv).toMatchObject({ style: { fill: "#fff" }, layerId: "L1", geometry: null });
  });

  it("re-links the old entity, or unlinks when there was none", () => {
    const linked = pin("p", 0, 0, { entityId: "E1" });
    expect(inversePatch(linked, { geometry: null, style: null, entityId: "E2" })).toMatchObject({
      entityId: "E1",
    });
    expect(
      inversePatch(linked, { geometry: null, style: null, entityId: null, unlinkEntity: true }),
    ).toMatchObject({ entityId: "E1" });
    const bare = pin("q");
    expect(inversePatch(bare, { geometry: null, style: null, entityId: "E2" })).toMatchObject({
      entityId: null,
      unlinkEntity: true,
    });
  });

  it("undoes several updates in reverse order and skips unknown ids", () => {
    const action = buildUpdate(
      "Move",
      [pin("a"), pin("b")],
      [
        { id: "a", patch: { geometry: { kind: "pin", x: 1, y: 1 } } },
        { id: "gone", patch: { geometry: { kind: "pin", x: 1, y: 1 } } },
        { id: "b", patch: { geometry: { kind: "pin", x: 1, y: 1 } } },
      ],
    );
    expect(action?.do).toHaveLength(2);
    expect(action?.undo.map((o) => (o.op === "update" ? o.id : ""))).toEqual(["b", "a"]);
    expect(buildUpdate("x", [], [{ id: "a", patch: {} }])).toBeNull();
  });
});

describe("buildDelete", () => {
  it("recreates deleted objects with id, createdAt, style and link", () => {
    const p = pin("p", 3, 4, {
      createdAt: "2025-05-05T00:00:00.000Z",
      style: { color: "#abc" },
      entityId: "E1",
      layerId: "L2",
    });
    const action = buildDelete("Delete", [p], ["p"]);
    expect(action?.do).toEqual([{ op: "delete", id: "p" }]);
    expect(action?.undo).toEqual([
      {
        op: "create",
        object: {
          id: "p",
          layerId: "L2",
          geometry: { kind: "pin", x: 3, y: 4 },
          style: { color: "#abc" },
          entityId: "E1",
          createdAt: "2025-05-05T00:00:00.000Z",
        },
      },
    ]);
  });

  it("restores the pathId of labels detached by a path delete", () => {
    const river = obj("river", {
      kind: "path",
      closed: false,
      points: [
        { x: 0, y: 0 },
        { x: 10, y: 10 },
      ],
    });
    const label = obj("lbl", {
      kind: "label",
      x: 1,
      y: 1,
      text: "River Ashen",
      rotation: 0,
      pathId: "river",
    });
    const other = obj("o", { kind: "label", x: 0, y: 0, text: "x", rotation: 0, pathId: null });
    const action = buildDelete("Delete", [label, other, river], ["river"]);
    expect(action?.undo).toHaveLength(2);
    expect(action?.undo[0]).toMatchObject({ op: "create", object: { id: "river" } });
    expect(action?.undo[1]).toMatchObject({
      op: "update",
      id: "lbl",
      patch: { geometry: { pathId: "river" } },
    });
  });

  it("recreates paths before labels when both are deleted, without extra updates", () => {
    const river = obj("river", {
      kind: "path",
      closed: false,
      points: [
        { x: 0, y: 0 },
        { x: 1, y: 1 },
      ],
    });
    const label = obj("lbl", {
      kind: "label",
      x: 0,
      y: 0,
      text: "t",
      rotation: 0,
      pathId: "river",
    });
    const action = buildDelete("Delete", [label, river], ["lbl", "river"]);
    expect(action?.undo.map((o) => (o.op === "create" ? o.object.id : o.op))).toEqual([
      "river",
      "lbl",
    ]);
  });

  it("returns null when nothing matches", () => {
    expect(buildDelete("Delete", [pin("a")], ["zzz"])).toBeNull();
  });
});

describe("applyOpsToData", () => {
  const base = (objects: MapObject[]): MapData =>
    ({ map: {}, layers: [], objects }) as unknown as MapData;

  it("creates, updates and deletes in order, and detaches labels", () => {
    const river = obj("river", { kind: "path", closed: false, points: [] as never });
    const label = obj("lbl", {
      kind: "label",
      x: 0,
      y: 0,
      text: "t",
      rotation: 0,
      pathId: "river",
    });
    const data = base([river, label, pin("p")]);
    const created = toNewObject({ layerId: "L1", geometry: { kind: "pin", x: 7, y: 7 } });
    const next = applyOpsToData(data, [
      { op: "create", object: created },
      {
        op: "update",
        id: "p",
        patch: { geometry: { kind: "pin", x: 9, y: 9 }, style: null, entityId: null },
      },
      { op: "delete", id: "river" },
    ]);
    expect(next.objects.map((o) => o.id)).toEqual(["lbl", "p", created.id]);
    expect(next.objects.find((o) => o.id === "p")?.geometry).toEqual({ kind: "pin", x: 9, y: 9 });
    const g = next.objects.find((o) => o.id === "lbl")?.geometry;
    expect(g?.kind === "label" && g.pathId).toBeNull();
  });

  it("an undo batch restores the exact prior state", () => {
    const river = obj("river", { kind: "path", closed: false, points: [] as never });
    const label = obj("lbl", {
      kind: "label",
      x: 0,
      y: 0,
      text: "t",
      rotation: 0,
      pathId: "river",
    });
    const data = base([river, label]);
    const action = buildDelete("Delete", data.objects, ["river"]);
    if (!action) throw new Error("expected an action");
    const afterDo = applyOpsToData(data, action.do);
    const afterUndo = applyOpsToData(afterDo, action.undo);
    const byId = new Map(afterUndo.objects.map((o) => [o.id, o]));
    expect(byId.get("river")?.createdAt).toBe(river.createdAt);
    const g = byId.get("lbl")?.geometry;
    expect(g?.kind === "label" && g.pathId).toBe("river");
  });
});
