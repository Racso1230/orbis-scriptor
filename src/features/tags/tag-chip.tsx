import { X } from "lucide-react";
import { cn } from "@/lib/utils";

/** A small pill for one tag. A tag colour tints the border and dot; the text stays readable. */
export function TagChip({
  name,
  color,
  onRemove,
  onClick,
  selected,
  className,
}: {
  name: string;
  color?: string | null;
  onRemove?: () => void;
  onClick?: () => void;
  selected?: boolean;
  className?: string;
}) {
  const body = (
    <>
      <span
        aria-hidden
        className="size-2 shrink-0 rounded-full bg-muted-foreground/50"
        style={color ? { backgroundColor: color } : undefined}
      />
      <span className="truncate">{name}</span>
    </>
  );
  const base = cn(
    "inline-flex max-w-full items-center gap-1.5 rounded-full border px-2 py-0.5 text-xs",
    selected && "bg-accent font-medium",
    className,
  );
  const style = color ? { borderColor: color } : undefined;
  return (
    <span className={base} style={style}>
      {onClick ? (
        <button
          type="button"
          onClick={onClick}
          aria-pressed={selected}
          className="inline-flex min-w-0 items-center gap-1.5"
        >
          {body}
        </button>
      ) : (
        body
      )}
      {onRemove && (
        <button
          type="button"
          aria-label={`Remove tag ${name}`}
          onClick={onRemove}
          className="rounded-full text-muted-foreground hover:text-foreground"
        >
          <X className="size-3" />
        </button>
      )}
    </span>
  );
}
