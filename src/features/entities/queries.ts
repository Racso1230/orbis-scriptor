import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type Entity, type EntityPatch, type NewEntity } from "@/lib/bindings";
import { searchKeys } from "@/features/search/queries";
import { unwrap } from "@/lib/unwrap";

export const entityKeys = {
  types: ["entities", "types"] as const,
  list: ["entities", "list"] as const,
  detail: (id: string) => ["entities", "detail", id] as const,
};

/** Built-in entity types with their field definitions (static for the session). */
export function useEntityTypes() {
  return useQuery({
    queryKey: entityKeys.types,
    // Not wrapped in a Result: this command cannot fail.
    queryFn: () => commands.entityTypes(),
    staleTime: Infinity,
  });
}

/** All entities of the open world (summaries, for the sidebar and tabs). */
export function useEntities() {
  return useQuery({
    queryKey: entityKeys.list,
    queryFn: () => unwrap(commands.listEntities(null)),
  });
}

export function useEntity(id: string) {
  return useQuery({
    queryKey: entityKeys.detail(id),
    queryFn: () => unwrap(commands.getEntity(id)),
  });
}

export function useCreateEntity() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: NewEntity) => unwrap(commands.createEntity(input)),
    onSuccess: (entity: Entity) => {
      queryClient.setQueryData(entityKeys.detail(entity.id), entity);
      return Promise.all([
        queryClient.invalidateQueries({ queryKey: entityKeys.list }),
        queryClient.invalidateQueries({ queryKey: searchKeys.all }),
        // A new `event` entity appears on the timeline (at day 0).
        queryClient.invalidateQueries({ queryKey: ["events"] }),
      ]);
    },
  });
}

export function useUpdateEntity() {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: ({ id, patch }: { id: string; patch: EntityPatch }) =>
      unwrap(commands.updateEntity(id, patch)),
    onSuccess: (entity: Entity) => {
      queryClient.setQueryData(entityKeys.detail(entity.id), entity);
      return Promise.all([
        queryClient.invalidateQueries({ queryKey: entityKeys.list }),
        queryClient.invalidateQueries({ queryKey: searchKeys.all }),
        // Pins show the linked entity's name and type.
        queryClient.invalidateQueries({ queryKey: ["maps"] }),
        // Timeline events show the entity name and vanish with the entity.
        queryClient.invalidateQueries({ queryKey: ["events"] }),
        // Relationship lists and graphs show the entity name and vanish with it.
        queryClient.invalidateQueries({ queryKey: ["relationships"] }),
        queryClient.invalidateQueries({ queryKey: ["graph"] }),
      ]);
    },
  });
}

export function useDeleteEntity() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.deleteEntity(id)),
    onSuccess: (_result, id) => {
      queryClient.removeQueries({ queryKey: entityKeys.detail(id) });
      return Promise.all([
        queryClient.invalidateQueries({ queryKey: entityKeys.list }),
        queryClient.invalidateQueries({ queryKey: searchKeys.all }),
        // Pins on maps show the linked entity's name and type.
        queryClient.invalidateQueries({ queryKey: ["maps"] }),
        // Timeline events show the entity name and vanish with the entity.
        queryClient.invalidateQueries({ queryKey: ["events"] }),
        // Relationship lists and graphs show the entity name and vanish with it.
        queryClient.invalidateQueries({ queryKey: ["relationships"] }),
        queryClient.invalidateQueries({ queryKey: ["graph"] }),
      ]);
    },
  });
}
