import { MarkerType, type Edge, type Node } from "@xyflow/react";
import type { GraphEdge, GraphFilter, RelationshipKindDef } from "@/lib/bindings";
import { CATEGORY_COLORS } from "./kinds";
import type { Position } from "./layout";

/** What a graph node needs to render (shared by the graph and family modes). */
export interface FlowEntity {
  id: string;
  name: string;
  type: string;
}

/** Data carried by every node; the custom node component reads it. */
export interface EntityNodeData extends Record<string, unknown> {
  name: string;
  entityType: string;
  isFocus: boolean;
}

export type EntityFlowNode = Node<EntityNodeData, "entity">;

export function toFlowNodes(
  entities: FlowEntity[],
  positions: Map<string, Position>,
  focusId: string | null,
): EntityFlowNode[] {
  return entities.map((e) => ({
    id: e.id,
    type: "entity",
    position: positions.get(e.id) ?? { x: 0, y: 0 },
    data: { name: e.name, entityType: e.type, isFocus: e.id === focusId },
    draggable: false,
  }));
}

/** Kinds drawn dashed because they are weak links, not real relationships. */
const WEAK_KINDS = new Set(["mention", "involved_in", "field_ref"]);

export interface EdgeOptions {
  /** Draw the relationship text on every edge (false: only on `hoveredId`). */
  showLabels: boolean;
  hoveredId: string | null;
}

/** Style an edge by category; symmetric edges have no arrowhead. */
export function toFlowEdges(edges: GraphEdge[], opts: EdgeOptions): Edge[] {
  return edges.map((e) => {
    const color = CATEGORY_COLORS[e.category];
    const hovered = opts.hoveredId === e.id;
    return {
      id: e.id,
      source: e.source,
      target: e.target,
      // Left-to-right layout: leave a node on its right side, enter on its left.
      sourceHandle: "r",
      targetHandle: "l",
      label: opts.showLabels || hovered ? e.label : undefined,
      type: "default",
      markerEnd: e.symmetric ? undefined : { type: MarkerType.ArrowClosed, color },
      style: {
        stroke: color,
        strokeWidth: hovered ? 3 : 1.5,
        strokeDasharray: WEAK_KINDS.has(e.kind) ? "5 4" : undefined,
      },
      labelStyle: { fontSize: 11, fill: "currentColor" },
      labelBgPadding: [4, 2] as [number, number],
      labelBgBorderRadius: 4,
      labelBgStyle: { fill: "var(--background)", fillOpacity: 0.9 },
      interactionWidth: 16,
    };
  });
}

/**
 * Family-tree edges: parent to child as an orthogonal step from the bottom of
 * the parent to the top of the child, spouses as a straight line between the
 * side handles. Sibling edges are omitted: shared parents already show them.
 */
export function toFamilyEdges(
  edges: GraphEdge[],
  positions: Map<string, Position>,
  opts: EdgeOptions,
): Edge[] {
  const out: Edge[] = [];
  for (const e of toFlowEdges(
    edges.filter((e) => e.kind !== "sibling_of"),
    opts,
  )) {
    const source = positions.get(e.source);
    const target = positions.get(e.target);
    const original = edges.find((x) => x.id === e.id);
    if (original?.kind === "spouse_of" && source && target) {
      // Left-most person's right handle to the other's left handle.
      const [left, right] = source.x <= target.x ? [e.source, e.target] : [e.target, e.source];
      out.push({
        ...e,
        source: left,
        target: right,
        sourceHandle: "r",
        targetHandle: "l",
        type: "straight",
        markerEnd: undefined,
      });
    } else {
      out.push({ ...e, sourceHandle: "b", targetHandle: "t", type: "smoothstep" });
    }
  }
  return out;
}

/** Cheap string that changes when the laid-out nodes change (used to refit the view). */
export function layoutKey(nodes: { id: string; position: Position }[]): string {
  let h = 5381;
  for (const n of nodes) {
    const s = `${n.id}@${Math.round(n.position.x)},${Math.round(n.position.y)};`;
    for (let i = 0; i < s.length; i++) h = ((h << 5) + h + s.charCodeAt(i)) | 0;
  }
  return `${nodes.length}:${h}`;
}

/** The weak link kinds the filter can opt into. */
export const MENTION_KIND = "mention";
export const INVOLVED_KIND = "involved_in";
export const FIELD_REF_KIND = "field_ref";

export interface FilterSelection {
  hiddenKinds: string[];
  hiddenTypes: string[];
  includeMentions: boolean;
  includeInvolved: boolean;
  /** Links made by entity-reference fields. */
  includeFieldRefs: boolean;
}

/**
 * Turns the checkbox state into the backend's `GraphFilter`.
 * `null` means "everything" for kinds and types; an empty kinds list means
 * "follow nothing" (only the focus is shown).
 */
export function buildFilter(
  relationshipKinds: RelationshipKindDef[],
  entityTypeKeys: string[],
  sel: FilterSelection,
): GraphFilter {
  const visibleKinds = relationshipKinds
    .map((k) => k.key)
    .filter((k) => !sel.hiddenKinds.includes(k));
  const extras = [
    ...(sel.includeMentions ? [MENTION_KIND] : []),
    ...(sel.includeInvolved ? [INVOLVED_KIND] : []),
    ...(sel.includeFieldRefs ? [FIELD_REF_KIND] : []),
  ];
  const allKindsVisible = visibleKinds.length === relationshipKinds.length;
  const visibleTypes = entityTypeKeys.filter((t) => !sel.hiddenTypes.includes(t));
  return {
    kinds: allKindsVisible && extras.length === 0 ? null : [...visibleKinds, ...extras],
    entityTypes: visibleTypes.length === entityTypeKeys.length ? null : visibleTypes,
    maxNodes: null,
  };
}
