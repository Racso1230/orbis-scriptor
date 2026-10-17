/**
 * "Save now" registry. Every mounted `useAutosave` registers a flush function
 * here, so a global shortcut (Ctrl+S) can force all pending edits to disk
 * without knowing which editors are on screen.
 */
export type FlushResult = "clean" | "saved" | "error";
type Flush = () => Promise<FlushResult>;

const flushers = new Set<Flush>();

/** Registers a flush function; returns the function that unregisters it. */
export function registerFlush(flush: Flush): () => void {
  flushers.add(flush);
  return () => {
    flushers.delete(flush);
  };
}

/** Flushes every registered autosave. Resolves to the combined outcome. */
export async function flushAll(): Promise<FlushResult> {
  const results = await Promise.all([...flushers].map((flush) => flush()));
  if (results.includes("error")) return "error";
  return results.includes("saved") ? "saved" : "clean";
}
