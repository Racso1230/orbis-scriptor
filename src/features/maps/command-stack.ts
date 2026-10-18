/**
 * A pure undo/redo stack. Nothing here knows about maps or React: callers keep
 * the returned stack in state and decide what "undo an entry" means.
 */
export interface CommandStack<T> {
  /** Oldest first. The last element is what Undo takes. */
  past: T[];
  /** Next redo is the LAST element (undone entries are pushed on the end). */
  future: T[];
}

/** Keep at most this many undoable steps per map. */
export const MAX_HISTORY = 200;

export function emptyStack<T>(): CommandStack<T> {
  return { past: [], future: [] };
}

/** Records a new action. Anything that could have been redone is discarded. */
export function pushEntry<T>(
  stack: CommandStack<T>,
  entry: T,
  limit: number = MAX_HISTORY,
): CommandStack<T> {
  const past = [...stack.past, entry];
  return { past: past.length > limit ? past.slice(past.length - limit) : past, future: [] };
}

/** Moves the newest entry from `past` to `future`; returns it so the caller can run its undo. */
export function popUndo<T>(stack: CommandStack<T>): { stack: CommandStack<T>; entry: T | null } {
  const entry = stack.past[stack.past.length - 1];
  if (entry === undefined) return { stack, entry: null };
  return {
    stack: { past: stack.past.slice(0, -1), future: [...stack.future, entry] },
    entry,
  };
}

/** Moves the next entry from `future` back to `past`; returns it so the caller can run its do. */
export function popRedo<T>(stack: CommandStack<T>): { stack: CommandStack<T>; entry: T | null } {
  const entry = stack.future[stack.future.length - 1];
  if (entry === undefined) return { stack, entry: null };
  return {
    stack: { past: [...stack.past, entry], future: stack.future.slice(0, -1) },
    entry,
  };
}

/** Drops matching entries from both sides, e.g. after the server rejected one. */
export function removeEntry<T>(
  stack: CommandStack<T>,
  matches: (entry: T) => boolean,
): CommandStack<T> {
  return {
    past: stack.past.filter((e) => !matches(e)),
    future: stack.future.filter((e) => !matches(e)),
  };
}

export function canUndo<T>(stack: CommandStack<T>): boolean {
  return stack.past.length > 0;
}

export function canRedo<T>(stack: CommandStack<T>): boolean {
  return stack.future.length > 0;
}
