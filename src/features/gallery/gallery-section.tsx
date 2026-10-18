import { useState } from "react";
import { ArrowLeft, ArrowRight, ImagePlus, Star, StarOff, Trash2 } from "lucide-react";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { EntityImage } from "@/lib/bindings";
import { fileSrc } from "@/lib/file-src";
import { moveId, thumbOrFull } from "./gallery-order";
import { useThumbRetry } from "./use-thumb-retry";
import { pickImageFiles } from "./pick-images";
import {
  useAddEntityImages,
  useRemoveImage,
  useReorderImages,
  useSetPortrait,
  useUpdateImageCaption,
} from "./queries";

const MAX_IMAGES = 100;
function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** The "Gallery" section of an entity page: thumbnails, captions, order and portrait. */
export function GallerySection({
  entityId,
  images,
  refetch,
  onOpen,
}: {
  entityId: string;
  images: EntityImage[];
  refetch: () => unknown;
  onOpen: (index: number) => void;
}) {
  const add = useAddEntityImages(entityId);
  const reorder = useReorderImages(entityId);
  const setPortrait = useSetPortrait(entityId);
  const remove = useRemoveImage(entityId);
  const caption = useUpdateImageCaption(entityId);
  const [error, setError] = useState<string | null>(null);
  const [removing, setRemoving] = useState<EntityImage | null>(null);
  useThumbRetry(images, refetch);

  async function run(job: () => Promise<unknown>) {
    setError(null);
    try {
      await job();
    } catch (e) {
      setError(messageOf(e));
    }
  }

  async function handleAdd() {
    const paths = await pickImageFiles();
    if (paths.length === 0) return;
    await run(() => add.mutateAsync(paths));
  }

  const ids = images.map((i) => i.id);

  return (
    <section aria-label="Gallery" className="flex flex-col gap-3">
      <div className="flex items-center gap-2">
        <h2 className="text-sm font-medium">Gallery</h2>
        {images.length > 0 && (
          <span className="text-xs text-muted-foreground tabular-nums">{images.length}</span>
        )}
        <Button
          variant="outline"
          size="sm"
          className="ml-auto"
          disabled={add.isPending || images.length >= MAX_IMAGES}
          onClick={() => void handleAdd()}
        >
          <ImagePlus /> Add images…
        </Button>
      </div>
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      {images.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          No images yet. Add portraits, sketches or reference pictures.
        </p>
      ) : (
        <ul className="grid grid-cols-2 gap-3 sm:grid-cols-3" aria-label="Gallery images">
          {images.map((image, index) => (
            <li key={image.id} className="flex flex-col gap-1.5 rounded-lg border p-2">
              <button
                type="button"
                onClick={() => onOpen(index)}
                aria-label={`Open ${image.caption || image.fileName}`}
                className="relative aspect-square w-full overflow-hidden rounded-md bg-muted"
              >
                <img
                  src={fileSrc(thumbOrFull(image))}
                  alt={image.caption || image.fileName}
                  loading="lazy"
                  className="size-full object-cover"
                />
                {image.isPortrait && (
                  <span className="absolute top-1 left-1 rounded-full bg-primary px-2 py-0.5 text-[10px] font-medium text-primary-foreground">
                    Portrait
                  </span>
                )}
              </button>
              <CaptionField
                // New key when the saved caption changes, so the field restarts from it.
                key={`${image.id}:${image.caption}`}
                image={image}
                onSave={(text) => run(() => caption.mutateAsync({ id: image.id, caption: text }))}
              />
              <div className="flex items-center gap-0.5">
                <Button
                  variant="ghost"
                  size="icon-xs"
                  aria-label={`Move ${image.fileName} earlier`}
                  disabled={index === 0 || reorder.isPending}
                  onClick={() => void run(() => reorder.mutateAsync(moveId(ids, index, -1)))}
                >
                  <ArrowLeft />
                </Button>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  aria-label={`Move ${image.fileName} later`}
                  disabled={index === images.length - 1 || reorder.isPending}
                  onClick={() => void run(() => reorder.mutateAsync(moveId(ids, index, 1)))}
                >
                  <ArrowRight />
                </Button>
                <Button
                  variant="ghost"
                  size="xs"
                  className="ml-auto"
                  disabled={setPortrait.isPending}
                  onClick={() =>
                    void run(() => setPortrait.mutateAsync(image.isPortrait ? null : image.id))
                  }
                >
                  {image.isPortrait ? <StarOff /> : <Star />}
                  {image.isPortrait ? "Remove portrait" : "Set as portrait"}
                </Button>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  aria-label={`Remove ${image.fileName}`}
                  onClick={() => setRemoving(image)}
                >
                  <Trash2 />
                </Button>
              </div>
            </li>
          ))}
        </ul>
      )}
      <AlertDialog open={removing !== null} onOpenChange={(open) => !open && setRemoving(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Remove this image?</AlertDialogTitle>
            <AlertDialogDescription>
              {removing?.fileName} is taken out of the gallery. The file itself is deleted only if
              nothing else (another gallery or a map) uses it.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                const target = removing;
                setRemoving(null);
                if (target) void run(() => remove.mutateAsync(target.id));
              }}
            >
              Remove
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </section>
  );
}

/** Caption input that saves when you press Enter or leave the field, if the text changed. */
function CaptionField({
  image,
  onSave,
}: {
  image: EntityImage;
  onSave: (text: string) => Promise<unknown>;
}) {
  const [text, setText] = useState(image.caption);
  function commit() {
    if (text !== image.caption) void onSave(text);
  }
  return (
    <>
      <Label htmlFor={`caption-${image.id}`} className="sr-only">
        Caption of {image.fileName}
      </Label>
      <Input
        id={`caption-${image.id}`}
        value={text}
        maxLength={500}
        placeholder="Caption…"
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => e.key === "Enter" && commit()}
        className="h-7 text-xs"
      />
    </>
  );
}
