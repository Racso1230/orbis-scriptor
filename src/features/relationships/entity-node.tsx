import { memo } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import { ExternalLink } from "lucide-react";
import { EntityIcon } from "@/features/entities/entity-icon";
import { useEntityTypes } from "@/features/entities/queries";
import { useOpenEntity } from "@/features/entities/use-open-entity";
import { lookFor } from "@/features/entities/type-look";
import { cn } from "@/lib/utils";
import type { EntityFlowNode } from "./flow-model";
import { NODE_H, NODE_W } from "./layout";

// Handles are invisible anchors for edges: "t"/"b" are used by the family
// tree (parents above, children below), "l"/"r" by the graph and for spouses.
const HANDLE_CLASS = "!size-1 !border-0 !bg-transparent";

/**
 * One entity in the graph. Click (handled by the canvas) refocuses on it;
 * double-click or the small button opens its page.
 */
function EntityNodeImpl({ id, data }: NodeProps<EntityFlowNode>) {
  const types = useEntityTypes();
  const open = useOpenEntity();
  const type = types.data?.find((t) => t.key === data.entityType);
  const color = lookFor(type).color;

  return (
    <div
      style={{ width: NODE_W, height: NODE_H, borderColor: color }}
      title={`${data.name} (${type?.label ?? data.entityType}). Double-click to open.`}
      onDoubleClick={() => open(id)}
      className={cn(
        "group flex items-center gap-2 rounded-lg border-2 bg-card px-2.5 text-sm text-card-foreground shadow-sm",
        data.isFocus && "ring-4 ring-primary/40",
      )}
    >
      <Handle id="t" type="target" position={Position.Top} className={HANDLE_CLASS} />
      <Handle id="l" type="target" position={Position.Left} className={HANDLE_CLASS} />
      <Handle id="b" type="source" position={Position.Bottom} className={HANDLE_CLASS} />
      <Handle id="r" type="source" position={Position.Right} className={HANDLE_CLASS} />
      <EntityIcon icon={type?.icon ?? ""} className="size-4 shrink-0" style={{ color }} />
      <span className={cn("truncate", data.isFocus && "font-semibold")}>{data.name}</span>
      <button
        type="button"
        aria-label={`Open ${data.name}`}
        onClick={(e) => {
          e.stopPropagation();
          open(id);
        }}
        className="nodrag nopan ml-auto hidden shrink-0 rounded-sm p-0.5 text-muted-foreground group-hover:block hover:text-foreground focus-visible:block"
      >
        <ExternalLink className="size-3.5" />
      </button>
    </div>
  );
}

export const EntityNode = memo(EntityNodeImpl);
