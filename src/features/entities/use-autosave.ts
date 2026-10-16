import { useEffect, useRef, useState } from "react";

export type SaveStatus = "saved" | "dirty" | "saving" | "error";

/**
 * Saves `value` a short while after it stops changing.
 *
 * - Compares values by JSON, so pass plain data (strings, numbers, objects).
 * - Flushes a pending save when the component unmounts (e.g. switching tabs).
 * - Returns the status for the "Saved / Saving…" indicator.
 */
export function useAutosave<T>(
  value: T,
  save: (value: T) => Promise<unknown>,
  delay = 800,
): { status: SaveStatus; error: string | null } {
  const [status, setStatus] = useState<SaveStatus>("saved");
  const [error, setError] = useState<string | null>(null);

  // Refs survive re-renders without triggering them. They are updated in an
  // effect (not during render) so React can keep renders pure.
  const lastSaved = useRef(JSON.stringify(value));
  const latest = useRef(value);
  const saveRef = useRef(save);
  useEffect(() => {
    latest.current = value;
    saveRef.current = save;
  });

  useEffect(() => {
    const serialized = JSON.stringify(value);
    if (serialized === lastSaved.current) return;
    setStatus("dirty");

    const timer = setTimeout(async () => {
      setStatus("saving");
      try {
        await saveRef.current(latest.current);
        lastSaved.current = JSON.stringify(latest.current);
        setStatus("saved");
        setError(null);
      } catch (e) {
        setStatus("error");
        setError(e instanceof Error ? e.message : String(e));
      }
    }, delay);

    return () => clearTimeout(timer);
  }, [value, delay]);

  // On unmount, save whatever is still pending so nothing is lost.
  useEffect(() => {
    return () => {
      if (JSON.stringify(latest.current) !== lastSaved.current) {
        void saveRef.current(latest.current);
      }
    };
  }, []);

  return { status, error };
}
