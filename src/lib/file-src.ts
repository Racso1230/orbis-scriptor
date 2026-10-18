import { convertFileSrc } from "@tauri-apps/api/core";
import type { Portrait } from "@/lib/bindings";

/**
 * URL the webview can load for a file inside the world folder. Falls back to
 * the plain path when Tauri's bridge is not there (unit tests, a plain browser).
 */
export function fileSrc(path: string): string {
  try {
    return convertFileSrc(path);
  } catch {
    return path;
  }
}

/** The image to draw for a small portrait: the thumbnail when it exists. */
export function portraitSrc(portrait: Portrait): string {
  return fileSrc(portrait.thumbPath ?? portrait.path);
}
