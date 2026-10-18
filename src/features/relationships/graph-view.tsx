import { useMemo, useState } from "react";
import { Background, Controls, MiniMap, ReactFlow, ReactFlowProvider } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { ArrowLeft, Filter, Focus, Network, Users } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useCreateEntity, useEntities, useEntityTypes } from "@/features/entities/queries";
import { lookFor } from "@/features/entities/type-look";
import { EntityPickerDialog } from "@/features/maps/entity-picker-dialog";
import { cn } from "@/lib/utils";
import { EntityNode } from "./entity-node";
import {
  buildFilter,
  layoutKey,
  toFamilyEdges,
  toFlowEdges,
  toFlowNodes,
  type FlowEntity,
} from "./flow-model";
import { GraphFilters } from "./graph-filters";
import { useGraphStore } from "./graph-store";
import { layoutFamily, layoutGraph } from "./layout";
import { useEntityGraph, useFamilyTree, useRelationshipKinds } from "./queries";

// Defined once, outside the component: React Flow re-renders every node
// whenever these objects change identity.
const nodeTypes = { entity: EntityNode };

/** Show edge text always while the picture stays readable; otherwise only on hover. */
const LABEL_LIMIT = 30;
const FAMILY_GENERATIONS = 3;

/** Route component for `/world/graph`. */
export function GraphPage() {
  return (
    <ReactFlowProvider>
      <GraphCanvas />
    </ReactFlowProvider>
  );
}

function GraphCanvas() {
  const { focusId, depth, mode, history, hiddenKinds, hiddenTypes, includeMentions } =
    useGraphStore();
  const includeInvolved = useGraphStore((s) => s.includeInvolved);
  const setFocus = useGraphStore((s) => s.setFocus);
  const setDepth = useGraphStore((s) => s.setDepth);
  const setMode = useGraphStore((s) => s.setMode);
  const back = useGraphStore((s) => s.back);

  const entities = useEntities();
  const types = useEntityTypes();
  const kinds = useRelationshipKinds();
  const createEntity = useCreateEntity();
  const [showFilters, setShowFilters] = useState(false);
  const [picking, setPicking] = useState(false);
  const [hoveredEdge, setHoveredEdge] = useState<string | null>(null);

  const focusEntity = entities.data?.find((e) => e.id === focusId);
  const focusExists = focusEntity !== undefined;
  const familyOk = focusEntity?.type === "character";

  const filter = useMemo(
    () =>
      buildFilter(
        kinds.data ?? [],
        (types.data ?? []).map((t) => t.key),
        {
          hiddenKinds,
          hiddenTypes,
          includeMentions,
          includeInvolved,
        },
      ),
    [kinds.data, types.data, hiddenKinds, hiddenTypes, includeMentions, includeInvolved],
  );

  const graphQuery = useEntityGraph(
    mode === "graph" && focusExists ? focusId : null,
    depth,
    filter,
  );
  const familyQuery = useFamilyTree(
    mode === "family" && familyOk ? focusId : null,
    FAMILY_GENERATIONS,
    FAMILY_GENERATIONS,
  );

  const data = mode === "graph" ? graphQuery.data : familyQuery.data;
  const query = mode === "graph" ? graphQuery : familyQuery;

  // Layout is a pure function of the data; it only re-runs when the data changes.
  const layout = useMemo(() => {
    if (!data) return null;
    const entitiesInView: FlowEntity[] = data.nodes;
    const positions =
      mode === "graph"
        ? layoutGraph(data.nodes, data.edges)
        : layoutFamily(familyQuery.data?.nodes ?? [], data.edges);
    const nodes = toFlowNodes(entitiesInView, positions, focusId);
    return { nodes, positions, key: `${mode}:${layoutKey(nodes)}` };
  }, [data, mode, familyQuery.data?.nodes, focusId]);

  const edges = useMemo(() => {
    if (!data || !layout) return [];
    const opts = {
      showLabels: mode === "graph" && data.edges.length <= LABEL_LIMIT,
      hoveredId: hoveredEdge,
    };
    return mode === "graph"
      ? toFlowEdges(data.edges, opts)
      : toFamilyEdges(data.edges, layout.positions, opts);
  }, [data, layout, mode, hoveredEdge]);

  async function createAndFocus(type: string, name: string) {
    setPicking(false);
    const entity = await createEntity.mutateAsync({ type, name });
    setFocus(entity.id);
  }

  const truncated = data?.truncated === true;

  return (
    <div className="flex min-w-0 flex-1 flex-col">
      <div className="flex h-11 shrink-0 items-center gap-2 border-b px-3">
        <h1 className="text-sm font-semibold">Graph</h1>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Back to previous focus"
          disabled={history.length === 0}
          onClick={back}
        >
          <ArrowLeft />
        </Button>
        <Button variant="outline" size="sm" onClick={() => setPicking(true)}>
          <Focus /> {focusEntity ? focusEntity.name : "Choose focus…"}
        </Button>

        {mode === "graph" && (
          <div role="group" aria-label="Depth" className="flex items-center gap-1">
            <span className="text-sm text-muted-foreground">Depth</span>
            {[1, 2, 3, 4].map((d) => (
              <Button
                key={d}
                variant={depth === d ? "secondary" : "ghost"}
                size="icon-sm"
                aria-pressed={depth === d}
                aria-label={`Depth ${d}`}
                onClick={() => setDepth(d)}
              >
                {d}
              </Button>
            ))}
          </div>
        )}

        <div className="ml-auto flex items-center gap-1">
          <Button
            variant={mode === "graph" ? "secondary" : "ghost"}
            size="sm"
            aria-pressed={mode === "graph"}
            onClick={() => setMode("graph")}
          >
            <Network /> Graph
          </Button>
          <Button
            variant={mode === "family" ? "secondary" : "ghost"}
            size="sm"
            aria-pressed={mode === "family"}
            onClick={() => setMode("family")}
          >
            <Users /> Family tree
          </Button>
          {mode === "graph" && (
            <Button
              variant={showFilters ? "secondary" : "ghost"}
              size="sm"
              aria-expanded={showFilters}
              onClick={() => setShowFilters((s) => !s)}
            >
              <Filter /> Filters
            </Button>
          )}
        </div>
      </div>

      {mode === "graph" && showFilters && (
        <GraphFilters kinds={kinds.data ?? []} types={types.data ?? []} />
      )}

      {truncated && data && (
        <p role="status" className="shrink-0 border-b bg-muted/40 px-4 py-1.5 text-sm">
          Too many connections: showing the first {data.nodes.length} entities. Lower the depth or
          add filters to narrow it down.
        </p>
      )}

      <div className="relative min-h-0 flex-1">
        {focusId === null || (entities.data && !focusExists) ? (
          <EmptyNote>
            {focusId === null
              ? "Choose an entity to see how it connects to the rest of your world."
              : "That entity no longer exists. Choose another one."}
          </EmptyNote>
        ) : mode === "family" && !familyOk ? (
          <EmptyNote>
            The family tree works for characters. Focus a character to see their family, or switch
            to the graph view.
          </EmptyNote>
        ) : query.error ? (
          <EmptyNote error>{query.error.message}</EmptyNote>
        ) : !layout ? (
          <EmptyNote>Loading…</EmptyNote>
        ) : (
          <>
            <ReactFlow
              // A new key remounts the canvas, which re-runs `fitView` on every
              // new layout (refocus, depth, filters), not just the first load.
              key={layout.key}
              nodes={layout.nodes}
              edges={edges}
              nodeTypes={nodeTypes}
              fitView
              fitViewOptions={{ padding: 0.2, maxZoom: 1.2 }}
              minZoom={0.1}
              nodesDraggable={false}
              nodesConnectable={false}
              elementsSelectable={false}
              onlyRenderVisibleElements={layout.nodes.length > 100}
              onNodeClick={(_event, node) => setFocus(node.id)}
              onEdgeMouseEnter={(_event, edge) => setHoveredEdge(edge.id)}
              onEdgeMouseLeave={() => setHoveredEdge(null)}
              proOptions={{ hideAttribution: true }}
            >
              <Background />
              <Controls showInteractive={false} />
              <MiniMap
                pannable
                zoomable
                nodeColor={(node) =>
                  lookFor(types.data?.find((t) => t.key === String(node.data["entityType"] ?? "")))
                    .color
                }
              />
            </ReactFlow>
            {layout.nodes.length === 1 && (
              <p
                className={cn(
                  "pointer-events-none absolute inset-x-0 bottom-6 text-center text-sm",
                  "text-muted-foreground",
                )}
              >
                No connections match the current settings.
              </p>
            )}
          </>
        )}
      </div>

      <EntityPickerDialog
        open={picking}
        onOpenChange={setPicking}
        onPickEntity={(id) => {
          setPicking(false);
          setFocus(id);
        }}
        onCreateEntity={(type, name) => void createAndFocus(type, name)}
        title="Focus the graph on…"
        description="Choose the entity at the centre of the graph"
      />
    </div>
  );
}

function EmptyNote({ children, error }: { children: React.ReactNode; error?: boolean }) {
  return (
    <div
      className={cn(
        "flex h-full items-center justify-center p-8 text-center text-sm",
        error ? "text-destructive" : "text-muted-foreground",
      )}
    >
      {children}
    </div>
  );
}
