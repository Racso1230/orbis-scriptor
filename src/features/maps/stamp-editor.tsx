import { Button } from "@/components/ui/button";
import type { MapGeometry } from "@/lib/bindings";
import { useMapActions } from "./map-actions-context";
import { isHexColor } from "./object-style";
import type { PropertyEditorProps } from "./property-editor-types";
import {
  DEFAULT_STAMP_INK,
  MAX_STAMP_SCALE,
  MIN_STAMP_SCALE,
  STAMP_CATEGORIES,
  stampByKey,
  stampName,
  stampsIn,
} from "./stamp-library";
import { StampSvg } from "./stamp-svg";
import { ColorControl, RangeControl } from "./style-controls";

type StampGeometry = Extract<MapGeometry, { kind: "stamp" }>;

/** Properties of a stamp: which icon, size, rotation and an optional colour tint. */
export function StampEditor({ object, disabled, onStyleChange }: PropertyEditorProps) {
  const actions = useMapActions();
  const g = object.geometry;
  if (g.kind !== "stamp") return null;
  const tint = isHexColor(object.style["color"]) ? object.style["color"] : null;

  function patchGeometry(label: string, changes: Partial<Omit<StampGeometry, "kind">>) {
    actions.update(label, [{ id: object.id, patch: { geometry: { ...g, ...changes } } }]);
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <StampSvg icon={g.icon} color={tint} size={36} />
        <label className="flex min-w-0 flex-1 items-center justify-between gap-2 text-xs">
          <span className="font-medium">Icon</span>
          <select
            aria-label="Icon"
            value={g.icon}
            disabled={disabled}
            onChange={(e) => patchGeometry("Change stamp icon", { icon: e.target.value })}
            className="h-7 min-w-0 flex-1 rounded-md border bg-background px-1.5 text-xs"
          >
            {!stampByKey(g.icon) && <option value={g.icon}>{stampName(g.icon)}</option>}
            {STAMP_CATEGORIES.map((category) => (
              <optgroup key={category.id} label={category.label}>
                {stampsIn(category.id).map((s) => (
                  <option key={s.key} value={s.key}>
                    {s.name}
                  </option>
                ))}
              </optgroup>
            ))}
          </select>
        </label>
      </div>
      <RangeControl
        label="Size"
        value={g.scale}
        min={MIN_STAMP_SCALE}
        max={Math.max(MAX_STAMP_SCALE, g.scale)}
        step={0.1}
        disabled={disabled}
        format={(v) => `${v.toFixed(1)}×`}
        onCommit={(scale) => patchGeometry("Resize stamp", { scale })}
      />
      <RangeControl
        label="Rotation"
        value={g.rotation}
        min={-180}
        max={180}
        step={5}
        disabled={disabled}
        format={(v) => `${v}°`}
        onCommit={(rotation) => patchGeometry("Rotate stamp", { rotation })}
      />
      <ColorControl
        label="Colour"
        value={tint ?? DEFAULT_STAMP_INK}
        disabled={disabled}
        onCommit={(color) => onStyleChange("Tint stamp", { color })}
      />
      {tint && (
        <Button
          variant="ghost"
          size="xs"
          disabled={disabled}
          onClick={() => onStyleChange("Reset stamp colour", { color: null })}
        >
          Reset colour
        </Button>
      )}
    </div>
  );
}
