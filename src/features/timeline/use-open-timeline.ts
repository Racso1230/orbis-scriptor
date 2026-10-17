import { useCallback } from "react";
import { useNavigate } from "@tanstack/react-router";
import { useTimelineStore, type TimelineFocus } from "./timeline-store";

/** Navigates to the timeline, optionally framing a day range. */
export function useOpenTimeline() {
  const navigate = useNavigate();
  const setPendingFocus = useTimelineStore((s) => s.setPendingFocus);

  return useCallback(
    (focus?: TimelineFocus) => {
      setPendingFocus(focus ?? null);
      void navigate({ to: "/world/timeline" });
    },
    [navigate, setPendingFocus],
  );
}
