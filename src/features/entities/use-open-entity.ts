import { useCallback } from "react";
import { useNavigate } from "@tanstack/react-router";
import { useCurrentWorld } from "@/features/worlds/queries";
import { useTabsStore } from "@/stores/tabs-store";

/** Opens an entity in a tab and navigates to it. Shared by sidebar, mentions and backlinks. */
export function useOpenEntity() {
  const navigate = useNavigate();
  const world = useCurrentWorld();
  const openTab = useTabsStore((s) => s.openTab);
  const worldPath = world.data?.path;

  return useCallback(
    (entityId: string) => {
      if (worldPath) openTab(worldPath, entityId);
      void navigate({ to: "/world/entity/$id", params: { id: entityId } });
    },
    [worldPath, openTab, navigate],
  );
}
