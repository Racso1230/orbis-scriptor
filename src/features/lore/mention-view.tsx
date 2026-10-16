import { NodeViewWrapper, type NodeViewProps } from "@tiptap/react";
import { useEntities } from "@/features/entities/queries";
import { useOpenEntity } from "@/features/entities/use-open-entity";
import { cn } from "@/lib/utils";

/** Renders a mention from the live entity list; red when the target is gone. */
export function MentionView({ node }: NodeViewProps) {
  const id = node.attrs.id as string;
  const entities = useEntities();
  const open = useOpenEntity();
  const entity = entities.data?.find((e) => e.id === id);
  const missing = entities.isSuccess && !entity;

  return (
    <NodeViewWrapper
      as="span"
      className={cn("mention", missing && "mention-missing")}
      data-mention-id={id}
      title={missing ? "This entity no longer exists" : "Open"}
      onClick={() => entity && open(id)}
    >
      {entity?.name ?? (missing ? "missing entity" : "…")}
    </NodeViewWrapper>
  );
}
