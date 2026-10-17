import { useCallback } from "react";
import { useNavigate, useParams } from "@tanstack/react-router";
import { useCurrentWorld } from "@/features/worlds/queries";
import { useTabsStore } from "@/stores/tabs-store";

/** Closes a tab; if it was the active one, moves to its neighbour (or the world home). */
export function useCloseTab() {
  const navigate = useNavigate();
  const { id: activeId } = useParams({ strict: false });
  const worldPath = useCurrentWorld().data?.path;
  const closeTab = useTabsStore((s) => s.closeTab);

  return useCallback(
    (id: string) => {
      if (!worldPath) return;
      const next = closeTab(worldPath, id);
      if (id !== activeId) return;
      if (next) void navigate({ to: "/world/entity/$id", params: { id: next } });
      else void navigate({ to: "/world" });
    },
    [worldPath, closeTab, activeId, navigate],
  );
}
