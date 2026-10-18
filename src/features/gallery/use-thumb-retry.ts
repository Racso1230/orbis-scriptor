import { useEffect, useRef } from "react";
import type { EntityImage } from "@/lib/bindings";
import { hasPendingThumbs } from "./gallery-order";

/** Thumbnails are made in the background; look again a few times until they exist. */
const THUMB_RETRIES = 6;
const THUMB_RETRY_MS = 1500;

/** Asks `refetch` again (a few times) while some thumbnail has not been generated yet. */
export function useThumbRetry(images: readonly EntityImage[], refetch: () => unknown) {
  const tries = useRef(0);
  const pending = hasPendingThumbs(images);
  useEffect(() => {
    if (!pending) {
      tries.current = 0;
      return;
    }
    if (tries.current >= THUMB_RETRIES) return;
    const id = window.setTimeout(() => {
      tries.current += 1;
      void refetch();
    }, THUMB_RETRY_MS);
    return () => window.clearTimeout(id);
  }, [pending, images, refetch]);
}
