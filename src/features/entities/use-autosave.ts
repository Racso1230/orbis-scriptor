import { useCallback, useEffect, useRef, useState } from "react";
import { registerFlush, type FlushResult } from "./save-registry";

export type SaveStatus = "saved" | "dirty" | "saving" | "error";

/**
 * Saves `value` a short while after it stops changing.
 *
 * - Compares values by JSON, so pass plain data (strings, numbers, objects).
 * - Flushes a pending save when the component unmounts (e.g. switching tabs).
 * - Registers a `flush` with the save registry so Ctrl+S can save immediately.
 * - Returns the status for the "Saved / Saving…" indicator.
 */
export function useAutosave<T>(
  value: T,
  save: (value: T) => Promise<unknown>,
  delay = 800,
): { status: SaveStatus; error: string | null; flush: () => Promise<FlushResult> } {
  const [status, setStatus] = useState<SaveStatus>("saved");
  const [error, setError] = useState<string | null>(null);

  // Refs survive re-renders without triggering them. They are updated in an
  // effect (not during render) so React can keep renders pure.
  const lastSaved = useRef(JSON.stringify(value));
  const latest = useRef(value);
  const saveRef = useRef(save);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => {
    latest.current = value;
    saveRef.current = save;
  });

  // Saves the latest value now (used by the timer and by `flush`).
  const saveNow = useCallback(async (): Promise<FlushResult> => {
    setStatus("saving");
    try {
      const saving = latest.current;
      await saveRef.current(saving);
      lastSaved.current = JSON.stringify(saving);
      setStatus("saved");
      setError(null);
      return "saved";
    } catch (e) {
      setStatus("error");
      setError(e instanceof Error ? e.message : String(e));
      return "error";
    }
  }, []);

  useEffect(() => {
    const serialized = JSON.stringify(value);
    if (serialized === lastSaved.current) return;
    setStatus("dirty");

    timer.current = setTimeout(() => void saveNow(), delay);

    return () => clearTimeout(timer.current);
  }, [value, delay, saveNow]);

  // Cancels the pending timer and saves right away if something is unsaved.
  const flush = useCallback(async (): Promise<FlushResult> => {
    clearTimeout(timer.current);
    if (JSON.stringify(latest.current) === lastSaved.current) return "clean";
    return saveNow();
  }, [saveNow]);

  useEffect(() => registerFlush(flush), [flush]);

  // On unmount, save whatever is still pending so nothing is lost.
  useEffect(() => {
    return () => {
      if (JSON.stringify(latest.current) !== lastSaved.current) {
        void saveRef.current(latest.current);
      }
    };
  }, []);

  return { status, error, flush };
}
