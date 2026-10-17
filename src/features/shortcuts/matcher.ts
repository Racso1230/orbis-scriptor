/** Actions the global keyboard shortcuts can trigger. */
export type ShortcutAction = "palette" | "newEntity" | "closeTab" | "save";

/** The parts of a KeyboardEvent the matcher needs (so tests can pass plain objects). */
export interface KeyInput {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

/** Single source of truth for the shortcut list (also rendered in Settings). */
export const SHORTCUTS: readonly { action: ShortcutAction; key: string; description: string }[] = [
  { action: "palette", key: "K", description: "Search and run commands" },
  { action: "newEntity", key: "N", description: "New entity" },
  { action: "closeTab", key: "W", description: "Close the current tab" },
  { action: "save", key: "S", description: "Save now" },
];

/**
 * Returns which shortcut a key press is, or null.
 * Ctrl (Windows/Linux) or Cmd (macOS) plus the letter; Alt or Shift make it
 * a different shortcut, so those are ignored.
 */
export function matchShortcut(e: KeyInput): ShortcutAction | null {
  if (!(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey) return null;
  const key = e.key.toLowerCase();
  return SHORTCUTS.find((s) => s.key.toLowerCase() === key)?.action ?? null;
}
