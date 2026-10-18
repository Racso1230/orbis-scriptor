import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type GraphFilter } from "@/lib/bindings";
import { unwrap } from "@/lib/unwrap";

export const relationshipKeys = {
  kinds: ["relationships", "kinds"] as const,
  /** Prefix for every per-entity relationship list. */
  all: ["relationships"] as const,
  forEntity: (entityId: string) => ["relationships", "entity", entityId] as const,
};

/** Prefix for all graph and family-tree queries (they are invalidated together). */
export const graphKeys = {
  all: ["graph"] as const,
  entity: (focusId: string, depth: number, filter: GraphFilter) =>
    ["graph", "entity", focusId, depth, filter] as const,
  family: (focusId: string, up: number, down: number) =>
    ["graph", "family", focusId, up, down] as const,
};

/** The relationship catalog: static for the session. */
export function useRelationshipKinds() {
  return useQuery({
    queryKey: relationshipKeys.kinds,
    // Not wrapped in a Result: this command cannot fail.
    queryFn: () => commands.listRelationshipKinds(),
    staleTime: Infinity,
  });
}

/** Relationships of one entity, phrased from its side ("child of Aldous"). */
export function useRelationships(entityId: string) {
  return useQuery({
    queryKey: relationshipKeys.forEntity(entityId),
    queryFn: () => unwrap(commands.listRelationships(entityId)),
  });
}

/** Relationship edits change both ends' lists and every graph, so refresh them all. */
function useInvalidateRelationships() {
  const queryClient = useQueryClient();
  return () =>
    Promise.all([
      queryClient.invalidateQueries({ queryKey: relationshipKeys.all }),
      queryClient.invalidateQueries({ queryKey: graphKeys.all }),
    ]);
}

export function useCreateRelationship() {
  const invalidate = useInvalidateRelationships();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (input: {
      sourceId: string;
      targetId: string;
      kind: string;
      label: string | null;
    }) =>
      unwrap(commands.createRelationship(input.sourceId, input.targetId, input.kind, input.label)),
    onSuccess: invalidate,
  });
}

export function useUpdateRelationship() {
  const invalidate = useInvalidateRelationships();
  return useMutation({
    meta: { inlineError: true },
    // `null` means "leave unchanged" for every field.
    mutationFn: (input: {
      linkId: string;
      kind?: string | null;
      label?: string | null;
      swapDirection?: boolean | null;
    }) =>
      unwrap(
        commands.updateRelationship(
          input.linkId,
          input.kind ?? null,
          input.label ?? null,
          input.swapDirection ?? null,
        ),
      ),
    onSuccess: invalidate,
  });
}

export function useDeleteRelationship() {
  const invalidate = useInvalidateRelationships();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (linkId: string) => unwrap(commands.deleteRelationship(linkId)),
    onSuccess: invalidate,
  });
}

/** Entities within `depth` hops of `focusId`. Keeps the old graph on screen while a new one loads. */
export function useEntityGraph(focusId: string | null, depth: number, filter: GraphFilter) {
  return useQuery({
    queryKey: graphKeys.entity(focusId ?? "", depth, filter),
    queryFn: () => unwrap(commands.getEntityGraph(focusId ?? "", depth, filter)),
    enabled: focusId !== null,
    placeholderData: keepPreviousData,
  });
}

export function useFamilyTree(focusId: string | null, up: number, down: number, enabled = true) {
  return useQuery({
    queryKey: graphKeys.family(focusId ?? "", up, down),
    queryFn: () => unwrap(commands.getFamilyTree(focusId ?? "", up, down)),
    enabled: focusId !== null && enabled,
    placeholderData: keepPreviousData,
  });
}

/** Dev builds only: creates N random relationships (the backend refuses in release). */
export function useSeedRelationships() {
  const invalidate = useInvalidateRelationships();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (count: number) => unwrap(commands.devSeedRelationships(count)),
    onSuccess: invalidate,
  });
}
