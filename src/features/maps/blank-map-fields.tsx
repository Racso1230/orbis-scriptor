import { useId } from "react";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { BackgroundKind } from "@/lib/bindings";
import { MAX_MAP_SIZE, MIN_MAP_SIZE, SIZE_PRESETS, type BlankMapForm } from "./blank-map";
import { BACKGROUND_LABELS, DEFAULT_PLAIN_COLOR } from "./map-background";

const KINDS: BackgroundKind[] = ["parchment", "ocean", "plain"];

export interface BlankMapFieldErrors {
  size?: string;
  color?: string;
}

interface Props {
  value: BlankMapForm;
  onChange: (next: BlankMapForm) => void;
  errors: BlankMapFieldErrors;
}

/** Size and background fields, shared by the "Blank map" and "Map settings" dialogs. */
export function BlankMapFields({ value, onChange, errors }: Props) {
  const uid = useId();
  const { background } = value;
  const tinted = background.color !== null;

  function setKind(kind: BackgroundKind) {
    // Plain needs a colour; the others fall back to "no tint".
    onChange({
      ...value,
      background: {
        kind,
        color: kind === "plain" ? (background.color ?? DEFAULT_PLAIN_COLOR) : background.color,
      },
    });
  }

  return (
    <div className="flex flex-col gap-4">
      <fieldset className="flex flex-col gap-1.5">
        <legend className="mb-1 text-sm font-medium">Size</legend>
        {SIZE_PRESETS.map((p) => (
          <label key={p.id} className="flex items-center gap-2 text-sm">
            <input
              type="radio"
              name={`${uid}-size`}
              checked={value.size === p.id}
              onChange={() => onChange({ ...value, size: p.id })}
            />
            {p.label}
          </label>
        ))}
        <label className="flex items-center gap-2 text-sm">
          <input
            type="radio"
            name={`${uid}-size`}
            checked={value.size === "custom"}
            onChange={() => onChange({ ...value, size: "custom" })}
          />
          Custom
        </label>
        {value.size === "custom" && (
          <div className="ml-6 flex items-center gap-2">
            <Input
              aria-label="Width in pixels"
              inputMode="numeric"
              className="w-24"
              value={value.customWidth}
              onChange={(e) => onChange({ ...value, customWidth: e.target.value })}
            />
            <span className="text-sm text-muted-foreground">×</span>
            <Input
              aria-label="Height in pixels"
              inputMode="numeric"
              className="w-24"
              value={value.customHeight}
              onChange={(e) => onChange({ ...value, customHeight: e.target.value })}
            />
            <span className="text-xs text-muted-foreground">
              px ({MIN_MAP_SIZE}–{MAX_MAP_SIZE})
            </span>
          </div>
        )}
        {errors.size && (
          <p role="alert" className="text-xs text-destructive">
            {errors.size}
          </p>
        )}
      </fieldset>

      <fieldset className="flex flex-col gap-1.5">
        <legend className="mb-1 text-sm font-medium">Background</legend>
        {KINDS.map((kind) => (
          <label key={kind} className="flex items-center gap-2 text-sm">
            <input
              type="radio"
              name={`${uid}-bg`}
              checked={background.kind === kind}
              onChange={() => setKind(kind)}
            />
            {BACKGROUND_LABELS[kind]}
          </label>
        ))}
        <div className="ml-6 flex items-center gap-2">
          {background.kind !== "plain" && (
            <label className="flex items-center gap-1.5 text-sm">
              <input
                type="checkbox"
                checked={tinted}
                onChange={(e) =>
                  onChange({
                    ...value,
                    background: {
                      kind: background.kind,
                      color: e.target.checked ? (background.color ?? "#c08a4a") : null,
                    },
                  })
                }
              />
              Tint
            </label>
          )}
          {(background.kind === "plain" || tinted) && (
            <>
              <Label htmlFor={`${uid}-color`} className="sr-only">
                {background.kind === "plain" ? "Colour" : "Tint colour"}
              </Label>
              <input
                id={`${uid}-color`}
                type="color"
                value={background.color ?? DEFAULT_PLAIN_COLOR}
                onChange={(e) =>
                  onChange({
                    ...value,
                    background: { kind: background.kind, color: e.target.value },
                  })
                }
                className="h-7 w-12 cursor-pointer rounded border bg-transparent p-0"
              />
            </>
          )}
        </div>
        {errors.color && (
          <p role="alert" className="text-xs text-destructive">
            {errors.color}
          </p>
        )}
      </fieldset>
    </div>
  );
}
