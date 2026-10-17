import { useEffect, useRef } from "react";
import { useParams } from "@tanstack/react-router";
import { toast } from "sonner";
import { flushAll } from "@/features/entities/save-registry";
import { useCloseTab } from "@/features/entities/use-close-tab";
import { useWorldTabs } from "@/stores/tabs-store";
import { useCurrentWorld } from "@/features/worlds/queries";
import { toastError } from "@/lib/notify";
import { matchShortcut } from "./matcher";

/** True while a modal dialog is open; N/W should not act behind it. */
function modalOpen(): boolean {
  return document.querySelector('[role="dialog"], [role="alertdialog"]') !== null;
}

/**
 * Global shortcuts for an open world: Ctrl+N (new entity), Ctrl+W (close tab),
 * Ctrl+S (save now). Ctrl+K is handled by the command palette itself.
 * The listener is on `window`, so it also fires while the TipTap editor or an
 * input has focus; we only claim these exact combinations (preventDefault).
 */
export function useGlobalShortcuts({ onNewEntity }: { onNewEntity: () => void }) {
  const { id: activeId } = useParams({ strict: false });
  const worldPath = useCurrentWorld().data?.path;
  const tabs = useWorldTabs(worldPath);
  const closeTab = useCloseTab();

  // Keep the latest values in a ref so the listener is attached only once.
  const latest = useRef({ activeId, tabs, closeTab, onNewEntity });
  useEffect(() => {
    latest.current = { activeId, tabs, closeTab, onNewEntity };
  });

  useEffect(() => {
    async function save() {
      const result = await flushAll();
      if (result === "error") toastError(new Error("Some changes could not be saved."), "save");
      else toast.success("Saved", { id: "save", duration: 1500 });
    }

    function onKeyDown(e: KeyboardEvent) {
      const action = matchShortcut(e);
      if (!action || action === "palette") return;
      const { activeId, tabs, closeTab, onNewEntity } = latest.current;

      if (action === "save") {
        e.preventDefault(); // stop the browser's "Save page" dialog
        void save();
      } else if (action === "newEntity") {
        e.preventDefault();
        if (!modalOpen()) onNewEntity();
      } else if (action === "closeTab") {
        e.preventDefault();
        if (!modalOpen() && activeId && tabs.includes(activeId)) closeTab(activeId);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
