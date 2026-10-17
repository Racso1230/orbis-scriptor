import { useState } from "react";
import { NewEntityDialog } from "./new-entity-dialog";
import { useGlobalShortcuts } from "./use-global-shortcuts";

/** Mounted once in the world layout: installs the shortcuts and owns their dialog. */
export function ShortcutsHost() {
  const [newOpen, setNewOpen] = useState(false);
  useGlobalShortcuts({ onNewEntity: () => setNewOpen(true) });
  return <NewEntityDialog open={newOpen} onOpenChange={setNewOpen} />;
}
