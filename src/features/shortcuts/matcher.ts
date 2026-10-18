/** Actions the global keyboard shortcuts can trigger. */
export type ShortcutAction = "palette" | "newEntity" | "closeTab" | "save" | "undo" | "redo";

/** The parts of a KeyboardEvent the matcher needs (so tests can pass plain objects). */
export interface KeyInput {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

/** Single source of truth for the shortcut list (also rendered in Settings). */
export const SHORTCUTS: readonly {
  action: ShortcutAction;
  key: string;
  description: string;
  /** Needs Shift as well (default: Shift must NOT be held). */
  shift?: boolean;
}[] = [
  { action: "palette", key: "K", description: "Search and run commands" },
  { action: "newEntity", key: "N", description: "New entity" },
  { action: "closeTab", key: "W", description: "Close the current tab" },
  { action: "save", key: "S", description: "Save now" },
  { action: "undo", key: "Z", description: "Undo (map editor)" },
  { action: "redo", key: "Z", shift: true, description: "Redo (map editor)" },
  { action: "redo", key: "Y", description: "Redo (map editor, alternative)" },
];

/**
 * Returns which shortcut a key press is, or null.
 * Ctrl (Windows/Linux) or Cmd (macOS) plus the letter. Alt makes it a different
 * shortcut, and so does Shift unless the shortcut asks for it (Redo).
 */
export function matchShortcut(e: KeyInput): ShortcutAction | null {
  if (!(e.ctrlKey || e.metaKey) || e.altKey) return null;
  const key = e.key.toLowerCase();
  return (
    SHORTCUTS.find((s) => s.key.toLowerCase() === key && (s.shift ?? false) === e.shiftKey)
      ?.action ?? null
  );
}
