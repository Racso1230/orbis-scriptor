/** Ids with the item at `index` moved by `delta` places (clamped), as a new array. */
export function moveId(ids: readonly string[], index: number, delta: number): string[] {
  const to = Math.min(ids.length - 1, Math.max(0, index + delta));
  const next = [...ids];
  const [item] = next.splice(index, 1);
  if (item === undefined) return next;
  next.splice(to, 0, item);
  return next;
}

/** The image to show for a thumbnail: the small file when ready, else the original. */
export function thumbOrFull(image: { path: string; thumbPath: string | null }): string {
  return image.thumbPath ?? image.path;
}

/** True while some image still waits for its thumbnail (generated in the background). */
export function hasPendingThumbs(images: readonly { thumbPath: string | null }[]): boolean {
  return images.some((i) => i.thumbPath === null);
}

/** Index after pressing previous/next in the lightbox; wraps around. */
export function stepIndex(index: number, delta: number, count: number): number {
  if (count === 0) return 0;
  return (((index + delta) % count) + count) % count;
}
