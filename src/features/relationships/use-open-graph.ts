import { useCallback } from "react";
import { useNavigate } from "@tanstack/react-router";
import { useGraphStore } from "./graph-store";

/** Navigates to the graph view, optionally focusing an entity ("Show in graph"). */
export function useOpenGraph() {
  const navigate = useNavigate();
  const setFocus = useGraphStore((s) => s.setFocus);

  return useCallback(
    (focusId?: string) => {
      if (focusId) setFocus(focusId);
      void navigate({ to: "/world/graph" });
    },
    [navigate, setFocus],
  );
}
