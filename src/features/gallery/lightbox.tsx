import { useEffect } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import type { EntityImage } from "@/lib/bindings";
import { fileSrc } from "@/lib/file-src";
import { stepIndex } from "./gallery-order";

/** Full-size viewer for gallery images. Esc closes, arrow keys (or the buttons) step through. */
export function Lightbox({
  images,
  index,
  onIndexChange,
  onClose,
}: {
  images: EntityImage[];
  /** `null` = closed. */
  index: number | null;
  onIndexChange: (index: number) => void;
  onClose: () => void;
}) {
  const image = index === null ? undefined : images[index];

  useEffect(() => {
    if (index === null) return;
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === "ArrowLeft") onIndexChange(stepIndex(index ?? 0, -1, images.length));
      else if (e.key === "ArrowRight") onIndexChange(stepIndex(index ?? 0, 1, images.length));
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [index, images.length, onIndexChange]);

  return (
    <Dialog open={image !== undefined} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="flex max-h-[90vh] flex-col items-center sm:max-w-4xl">
        <DialogTitle className="sr-only">Image viewer</DialogTitle>
        <DialogDescription className="sr-only">
          Use the arrow keys to move between images, Escape to close.
        </DialogDescription>
        {image && index !== null && (
          <>
            <img
              src={fileSrc(image.path)}
              alt={image.caption || image.fileName}
              className="max-h-[70vh] max-w-full rounded-md object-contain"
            />
            <p className="text-center text-sm">
              {image.caption || <span className="text-muted-foreground">{image.fileName}</span>}
            </p>
            <div className="flex items-center gap-3">
              <Button
                variant="outline"
                size="icon"
                aria-label="Previous image"
                disabled={images.length < 2}
                onClick={() => onIndexChange(stepIndex(index, -1, images.length))}
              >
                <ChevronLeft />
              </Button>
              <span className="text-xs text-muted-foreground tabular-nums">
                {index + 1} / {images.length}
              </span>
              <Button
                variant="outline"
                size="icon"
                aria-label="Next image"
                disabled={images.length < 2}
                onClick={() => onIndexChange(stepIndex(index, 1, images.length))}
              >
                <ChevronRight />
              </Button>
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
