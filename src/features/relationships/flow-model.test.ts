import { describe, expect, it } from "vitest";
import { buildFilter, toFamilyEdges, toFlowEdges } from "./flow-model";
import { edge, KINDS } from "./test-fixtures";

const base = { hiddenKinds: [], hiddenTypes: [], includeMentions: false, includeInvolved: false };
const types = ["character", "faction", "location"];

describe("buildFilter", () => {
  it("sends null (everything) when nothing is hidden", () => {
    expect(buildFilter(KINDS, types, base)).toEqual({
      kinds: null,
      entityTypes: null,
      maxNodes: null,
    });
  });

  it("lists the visible kinds when some are hidden", () => {
    const f = buildFilter(KINDS, types, { ...base, hiddenKinds: ["member_of", "custom"] });
    expect(f.kinds).toEqual(["parent_of", "spouse_of", "located_in", "related_to"]);
  });

  it("filters to family only", () => {
    const hidden = KINDS.filter((k) => k.category !== "family").map((k) => k.key);
    expect(buildFilter(KINDS, types, { ...base, hiddenKinds: hidden }).kinds).toEqual([
      "parent_of",
      "spouse_of",
    ]);
  });

  it("opts into mentions and involved_in explicitly", () => {
    const f = buildFilter(KINDS, types, { ...base, includeMentions: true });
    expect(f.kinds).toContain("mention");
    expect(f.kinds).toContain("parent_of");
    expect(f.kinds).not.toContain("involved_in");
  });

  it("sends an empty list when every kind is hidden", () => {
    const f = buildFilter(KINDS, types, { ...base, hiddenKinds: KINDS.map((k) => k.key) });
    expect(f.kinds).toEqual([]);
  });

  it("lists visible entity types when some are hidden", () => {
    expect(buildFilter(KINDS, types, { ...base, hiddenTypes: ["location"] }).entityTypes).toEqual([
      "character",
      "faction",
    ]);
  });
});

describe("toFlowEdges", () => {
  const opts = { showLabels: false, hoveredId: null };

  it("draws arrows only on directed edges", () => {
    const [directed, symmetric] = toFlowEdges(
      [edge("a", "x", "y", "parent_of"), edge("b", "x", "y", "spouse_of", true)],
      opts,
    );
    expect(directed?.markerEnd).toBeDefined();
    expect(symmetric?.markerEnd).toBeUndefined();
  });

  it("shows a label only when enabled or hovered", () => {
    const edges = [edge("a", "x", "y", "parent_of")];
    expect(toFlowEdges(edges, opts)[0]?.label).toBeUndefined();
    expect(toFlowEdges(edges, { ...opts, showLabels: true })[0]?.label).toBe("parent_of");
    expect(toFlowEdges(edges, { ...opts, hoveredId: "a" })[0]?.label).toBe("parent_of");
  });
});

describe("toFamilyEdges", () => {
  it("uses step edges for parents, side handles for spouses, and drops siblings", () => {
    const positions = new Map([
      ["p", { x: 200, y: 0 }],
      ["s", { x: 0, y: 0 }],
      ["c", { x: 100, y: 200 }],
    ]);
    const edges = toFamilyEdges(
      [
        edge("1", "p", "c", "parent_of"),
        edge("2", "p", "s", "spouse_of", true),
        edge("3", "s", "c", "sibling_of", true),
      ],
      positions,
      { showLabels: false, hoveredId: null },
    );
    expect(edges.map((e) => e.id)).toEqual(["1", "2"]);
    expect(edges[0]).toMatchObject({ type: "smoothstep", sourceHandle: "b", targetHandle: "t" });
    // The spouse edge is re-oriented left to right.
    expect(edges[1]).toMatchObject({
      source: "s",
      target: "p",
      sourceHandle: "r",
      targetHandle: "l",
    });
  });
});
