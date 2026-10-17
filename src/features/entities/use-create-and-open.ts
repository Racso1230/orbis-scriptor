import { useCallback } from "react";
import { useCreateEntity } from "./queries";
import { useOpenEntity } from "./use-open-entity";

/** Creates a "New <label>" entity of the given type and opens it in a tab. */
export function useCreateAndOpen() {
  const createEntity = useCreateEntity();
  const openEntity = useOpenEntity();
  const { mutateAsync } = createEntity;

  const createAndOpen = useCallback(
    async (typeKey: string, label: string) => {
      const entity = await mutateAsync({ type: typeKey, name: `New ${label}` });
      openEntity(entity.id);
    },
    [mutateAsync, openEntity],
  );
  return { createAndOpen, isPending: createEntity.isPending };
}
