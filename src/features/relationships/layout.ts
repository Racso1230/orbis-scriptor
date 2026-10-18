import { Graph, layout } from "@dagrejs/dagre";
import type { FamilyNode, GraphEdge } from "@/lib/bindings";

/** Size of every node box (matches the custom node component). */
export const NODE_W = 176;
export const NODE_H = 44;

/** Top-left corner of a node, in flow coordinates. */
export interface Position {
  x: number;
  y: number;
}

/**
 * Layered auto-layout for the general graph.
 *
 * We use dagre: it is synchronous and pure (nodes + edges in, positions out),
 * a few hundred nodes lay out in well under a second, and it is small and
 * easy to unit test. `rankdir: "LR"` reads like a network diagram.
 */
export function layoutGraph(
  nodes: { id: string }[],
  edges: { source: string; target: string }[],
): Map<string, Position> {
  const g = new Graph();
  g.setGraph({ rankdir: "LR", nodesep: 24, ranksep: 110, marginx: 0, marginy: 0 });
  g.setDefaultEdgeLabel(() => ({}));
  for (const n of nodes) g.setNode(n.id, { width: NODE_W, height: NODE_H });
  for (const e of edges) {
    // Skip anything pointing at a node we do not have (should not happen).
    if (g.hasNode(e.source) && g.hasNode(e.target) && e.source !== e.target) {
      g.setEdge(e.source, e.target);
    }
  }
  layout(g);

  const out = new Map<string, Position>();
  for (const n of nodes) {
    const p = g.node(n.id);
    // dagre reports the node centre; React Flow wants the top-left corner.
    out.set(n.id, { x: p.x - NODE_W / 2, y: p.y - NODE_H / 2 });
  }
  return out;
}

const SPOUSE_GAP = 24;
const SIBLING_GAP = 40;
const ROW_GAP = 90;

/**
 * Family-tree layout: one row per generation (oldest on top), spouses side by
 * side, and each unit pulled towards the middle of its parents.
 *
 * Algorithm, top row to bottom row:
 *  1. Group nodes of the same generation that are joined by `spouse_of` into
 *     "units" (a couple, or a single person).
 *  2. Wish x = average x of the unit's parents (already placed in the row above).
 *  3. Sort units by wish x and place them left to right, never overlapping.
 */
export function layoutFamily(nodes: FamilyNode[], edges: GraphEdge[]): Map<string, Position> {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const parentsOf = new Map<string, string[]>();
  const spouseLinks: [string, string][] = [];
  for (const e of edges) {
    if (!byId.has(e.source) || !byId.has(e.target)) continue;
    if (e.kind === "parent_of") {
      parentsOf.set(e.target, [...(parentsOf.get(e.target) ?? []), e.source]);
    } else if (e.kind === "spouse_of") {
      spouseLinks.push([e.source, e.target]);
    }
  }

  // Union-find: ids that share a root form one unit.
  const root = new Map(nodes.map((n) => [n.id, n.id]));
  const find = (id: string): string => {
    let r = id;
    while (root.get(r) !== r) r = root.get(r) ?? r;
    root.set(id, r);
    return r;
  };
  for (const [a, b] of spouseLinks) {
    if (byId.get(a)?.generation === byId.get(b)?.generation) root.set(find(a), find(b));
  }

  const generations = [...new Set(nodes.map((n) => n.generation))].sort((a, b) => a - b);
  const minGen = generations[0] ?? 0;
  const centreX = new Map<string, number>();
  const out = new Map<string, Position>();

  for (const gen of generations) {
    const units = new Map<string, FamilyNode[]>();
    for (const n of nodes.filter((n) => n.generation === gen)) {
      const key = find(n.id);
      units.set(key, [...(units.get(key) ?? []), n]);
    }

    const placed = [...units.values()].map((members) => {
      members.sort((a, b) => a.name.localeCompare(b.name));
      const parentXs = members
        .flatMap((m) => parentsOf.get(m.id) ?? [])
        .map((p) => centreX.get(p))
        .filter((x): x is number => x !== undefined);
      const wish = parentXs.length ? parentXs.reduce((s, x) => s + x, 0) / parentXs.length : null;
      return { members, wish, name: members[0]?.name ?? "" };
    });
    // Units with a known wish go in wish order; the rest follow, sorted by name.
    placed.sort((a, b) => {
      if (a.wish !== null && b.wish !== null) return a.wish - b.wish;
      if (a.wish !== null) return -1;
      if (b.wish !== null) return 1;
      return a.name.localeCompare(b.name);
    });

    // Units with the same wish are siblings: place them as one block centred
    // under their parents instead of starting at the parents' x.
    const blocks: (typeof placed)[] = [];
    for (const unit of placed) {
      const last = blocks[blocks.length - 1];
      if (last && unit.wish !== null && last[0]?.wish === unit.wish) last.push(unit);
      else blocks.push([unit]);
    }

    let cursor = Number.NEGATIVE_INFINITY;
    const y = (gen - minGen) * (NODE_H + ROW_GAP);
    const widthOf = (u: { members: FamilyNode[] }) =>
      u.members.length * NODE_W + (u.members.length - 1) * SPOUSE_GAP;
    for (const block of blocks) {
      const wish = block[0]?.wish ?? null;
      const total =
        block.reduce((sum, u) => sum + widthOf(u), 0) + (block.length - 1) * SIBLING_GAP;
      let left = wish === null ? Math.max(cursor, 0) : Math.max(cursor, wish - total / 2);
      for (const unit of block) {
        unit.members.forEach((m, i) => {
          const x = left + i * (NODE_W + SPOUSE_GAP);
          out.set(m.id, { x, y });
          centreX.set(m.id, x + NODE_W / 2);
        });
        left += widthOf(unit) + SIBLING_GAP;
      }
      cursor = left;
    }
  }

  // Shift everything so the leftmost node sits at x = 0.
  const minX = Math.min(0, ...[...out.values()].map((p) => p.x));
  if (minX < 0) for (const p of out.values()) p.x -= minX;
  return out;
}
