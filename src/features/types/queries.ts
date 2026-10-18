import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { entityKeys } from "@/features/entities/queries";
import { TYPE_ICONS } from "@/features/entities/entity-icon";
import { searchKeys } from "@/features/search/queries";
import { commands, type EntityTypeDef, type FieldInput } from "@/lib/bindings";
import { unwrap } from "@/lib/unwrap";

export const typeKeys = {
  icons: ["entities", "type-icons"] as const,
};

/** The curated icon names a type can use (static; not a `Result`, so no unwrap). */
export function useTypeIcons() {
  return useQuery({
    queryKey: typeKeys.icons,
    queryFn: () => commands.listTypeIcons(),
    staleTime: Infinity,
    // Until the backend answers, offer the names the frontend can draw.
    placeholderData: Object.keys(TYPE_ICONS),
  });
}

/**
 * The type list is cached with `staleTime: Infinity`, so every type mutation
 * must invalidate it. Deleting a type can also move entities to another type,
 * which touches lists, search, pins and graphs.
 */
function invalidateTypes(queryClient: QueryClient, entitiesMoved: boolean) {
  const jobs = [queryClient.invalidateQueries({ queryKey: entityKeys.types })];
  if (entitiesMoved) {
    jobs.push(
      queryClient.invalidateQueries({ queryKey: ["entities"] }),
      queryClient.invalidateQueries({ queryKey: searchKeys.all }),
      queryClient.invalidateQueries({ queryKey: ["maps"] }),
      queryClient.invalidateQueries({ queryKey: ["relationships"] }),
      queryClient.invalidateQueries({ queryKey: ["graph"] }),
    );
  }
  return Promise.all(jobs);
}

export function useCreateEntityType() {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (input: {
      label: string;
      plural: string;
      icon: string;
      color: string;
      fields: FieldInput[];
    }) =>
      unwrap(
        commands.createEntityType(input.label, input.plural, input.icon, input.color, input.fields),
      ),
    onSuccess: () => invalidateTypes(queryClient, false),
  });
}

export function useUpdateEntityType() {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (input: {
      key: string;
      label: string;
      plural: string;
      icon: string;
      color: string;
      fields: FieldInput[];
    }) =>
      unwrap(
        commands.updateEntityType(
          input.key,
          input.label,
          input.plural,
          input.icon,
          input.color,
          input.fields,
        ),
      ),
    onSuccess: () => invalidateTypes(queryClient, false),
  });
}

/** Extra user fields on a built-in type. */
export function useSetTypeTemplateFields() {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (input: { typeKey: string; fields: FieldInput[] }) =>
      unwrap(commands.setTypeTemplateFields(input.typeKey, input.fields)),
    onSuccess: () => invalidateTypes(queryClient, false),
  });
}

export function useDeleteEntityType() {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (input: { key: string; reassignTo: string | null }) =>
      unwrap(commands.deleteEntityType(input.key, input.reassignTo)),
    onSuccess: () => invalidateTypes(queryClient, true),
  });
}

/** `keys` lists every custom type once, in the new order. */
export function useReorderEntityTypes() {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (keys: string[]) => unwrap(commands.reorderEntityTypes(keys)),
    onSuccess: (types: EntityTypeDef[]) => {
      queryClient.setQueryData(entityKeys.types, types);
      return invalidateTypes(queryClient, false);
    },
  });
}
