import { cn } from "@/lib/utils";
import { useMapToolStore } from "./map-tool-store";
import { StampSvg } from "./stamp-svg";
import { STAMP_CATEGORIES, stampsIn } from "./stamp-library";

/**
 * The stamp picker shown over the map while the Stamp tool is active: every
 * stamp with a preview drawn from the same data as the canvas.
 */
export function StampPalette() {
  const icon = useMapToolStore((s) => s.stampIcon);
  const setIcon = useMapToolStore((s) => s.setStampIcon);

  return (
    <div
      role="group"
      aria-label="Stamps"
      className="absolute top-12 left-2 z-10 flex max-h-[calc(100%-4rem)] w-60 flex-col gap-2 overflow-y-auto rounded-md border bg-popover/95 p-2 text-popover-foreground shadow-md"
    >
      {STAMP_CATEGORIES.map((category) => (
        <div key={category.id} className="flex flex-col gap-1">
          <span className="text-xs font-semibold tracking-wide text-muted-foreground uppercase">
            {category.label}
          </span>
          <div className="grid grid-cols-4 gap-1">
            {stampsIn(category.id).map((stamp) => (
              <button
                key={stamp.key}
                type="button"
                aria-label={stamp.name}
                aria-pressed={icon === stamp.key}
                title={stamp.name}
                onClick={() => setIcon(stamp.key)}
                className={cn(
                  "flex items-center justify-center rounded-md border border-transparent p-1 hover:bg-accent",
                  icon === stamp.key && "border-ring bg-accent",
                )}
              >
                <StampSvg icon={stamp.key} size={40} />
              </button>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
