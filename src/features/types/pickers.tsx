import { useState } from "react";
import { Input } from "@/components/ui/input";
import { EntityIcon } from "@/features/entities/entity-icon";
import { cn } from "@/lib/utils";
import { COLOR_SWATCHES, isHexColor } from "./draft";

/** Searchable grid of the allow-listed icons. */
export function IconPicker({
  icons,
  value,
  color,
  onChange,
}: {
  icons: string[];
  value: string;
  color: string;
  onChange: (icon: string) => void;
}) {
  const [query, setQuery] = useState("");
  const needle = query.trim().toLowerCase();
  const shown = icons.filter((name) => name.includes(needle));
  return (
    <div className="flex flex-col gap-2">
      <Input
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Search icons…"
        aria-label="Search icons"
        className="h-8"
      />
      <div role="group" aria-label="Icon" className="grid grid-cols-8 gap-1">
        {shown.map((name) => (
          <button
            key={name}
            type="button"
            title={name}
            aria-label={name}
            aria-pressed={name === value}
            onClick={() => onChange(name)}
            className={cn(
              "flex size-9 items-center justify-center rounded-md border border-transparent hover:bg-accent",
              name === value && "border-ring bg-accent",
            )}
          >
            <EntityIcon icon={name} className="size-5" style={{ color }} />
          </button>
        ))}
        {shown.length === 0 && (
          <p className="col-span-8 text-sm text-muted-foreground">No icon matches.</p>
        )}
      </div>
    </div>
  );
}

/** Swatches plus a free `#rrggbb` field. */
export function ColorPicker({
  value,
  onChange,
}: {
  value: string;
  onChange: (color: string) => void;
}) {
  const valid = isHexColor(value);
  return (
    <div className="flex flex-col gap-2">
      <div role="group" aria-label="Colour swatches" className="flex flex-wrap gap-1.5">
        {COLOR_SWATCHES.map((swatch) => (
          <button
            key={swatch}
            type="button"
            aria-label={`Colour ${swatch}`}
            aria-pressed={swatch === value.toLowerCase()}
            onClick={() => onChange(swatch)}
            style={{ backgroundColor: swatch }}
            className={cn(
              "size-7 rounded-full border-2 border-transparent",
              swatch === value.toLowerCase() && "border-foreground",
            )}
          />
        ))}
      </div>
      <div className="flex items-center gap-2">
        <input
          type="color"
          aria-label="Pick a colour"
          value={valid ? value.toLowerCase() : "#000000"}
          onChange={(e) => onChange(e.target.value)}
          className="size-8 cursor-pointer rounded border bg-transparent p-0.5"
        />
        <Input
          aria-label="Colour hex"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          aria-invalid={!valid}
          maxLength={7}
          className="h-8 w-28 font-mono"
        />
        {!valid && <span className="text-xs text-destructive">Use #rrggbb.</span>}
      </div>
    </div>
  );
}
