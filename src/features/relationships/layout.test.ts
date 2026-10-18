import { describe, expect, it } from "vitest";
import type { FamilyNode } from "@/lib/bindings";
import { NODE_W, layoutFamily, layoutGraph } from "./layout";
import { edge } from "./test-fixtures";

describe("layoutGraph", () => {
  it("returns a finite position for every node, including isolated ones", () => {
    const nodes = ["a", "b", "c", "d"].map((id) => ({ id }));
    const positions = layoutGraph(nodes, [
      { source: "a", target: "b" },
      { source: "b", target: "c" },
      { source: "a", target: "ghost" }, // dangling edges are ignored
    ]);
    expect([...positions.keys()].sort()).toEqual(["a", "b", "c", "d"]);
    for (const p of positions.values()) {
      expect(Number.isFinite(p.x)).toBe(true);
      expect(Number.isFinite(p.y)).toBe(true);
    }
  });

  it("does not stack nodes on top of each other", () => {
    const nodes = Array.from({ length: 30 }, (_, i) => ({ id: `n${i}` }));
    const edges = nodes.slice(1).map((n) => ({ source: "n0", target: n.id }));
    const positions = layoutGraph(nodes, edges);
    const spots = new Set([...positions.values()].map((p) => `${p.x},${p.y}`));
    expect(spots.size).toBe(30);
  });

  it("handles an empty graph", () => {
    expect(layoutGraph([], []).size).toBe(0);
  });
});

const person = (id: string, generation: number, name = id): FamilyNode => ({
  id,
  name,
  type: "character",
  generation,
});

describe("layoutFamily", () => {
  // Grandpa (-1) -> Isolde (0) + spouse Lucan (0) -> Corwin (1)
  const nodes = [
    person("grandpa", -1),
    person("isolde", 0),
    person("lucan", 0),
    person("corwin", 1),
  ];
  const edges = [
    edge("e1", "grandpa", "isolde", "parent_of"),
    edge("e2", "isolde", "lucan", "spouse_of", true),
    edge("e3", "isolde", "corwin", "parent_of"),
    edge("e4", "lucan", "corwin", "parent_of"),
  ];
  const pos = layoutFamily(nodes, edges);
  const at = (id: string) => {
    const p = pos.get(id);
    if (!p) throw new Error(`no position for ${id}`);
    return p;
  };

  it("puts older generations higher up", () => {
    expect(at("grandpa").y).toBeLessThan(at("isolde").y);
    expect(at("isolde").y).toBeLessThan(at("corwin").y);
    expect(at("isolde").y).toBe(at("lucan").y);
  });

  it("places spouses adjacent on the same row", () => {
    const gap = Math.abs(at("isolde").x - at("lucan").x);
    expect(gap).toBeGreaterThanOrEqual(NODE_W);
    expect(gap).toBeLessThan(NODE_W * 1.5);
  });

  it("centres a child between its parents", () => {
    const lo = Math.min(at("isolde").x, at("lucan").x);
    const hi = Math.max(at("isolde").x, at("lucan").x);
    expect(at("corwin").x).toBeGreaterThanOrEqual(lo);
    expect(at("corwin").x).toBeLessThanOrEqual(hi);
  });

  it("never overlaps siblings and keeps x non-negative", () => {
    const family = [
      person("p", 0),
      person("k1", 1, "A"),
      person("k2", 1, "B"),
      person("k3", 1, "C"),
    ];
    const p = layoutFamily(
      family,
      family.slice(1).map((k, i) => edge(`e${i}`, "p", k.id, "parent_of")),
    );
    const xs = ["k1", "k2", "k3"].map((id) => p.get(id)?.x ?? Number.NaN).sort((a, b) => a - b);
    expect((xs[1] ?? 0) - (xs[0] ?? 0)).toBeGreaterThanOrEqual(NODE_W);
    expect((xs[2] ?? 0) - (xs[1] ?? 0)).toBeGreaterThanOrEqual(NODE_W);
    expect(xs[0]).toBeGreaterThanOrEqual(0);
    // The parent sits above the middle child.
    expect(p.get("p")?.x).toBeCloseTo(xs[1] ?? 0);
  });
});
