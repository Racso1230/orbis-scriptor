import { Button } from "@/components/ui/button";
import type { MapGeometry, MapObject } from "@/lib/bindings";
import { LABEL_FONT_OPTIONS, labelLook } from "./label-style";
import { useMapActions } from "./map-actions-context";
import type { PropertyEditorProps } from "./property-editor-types";
import { ColorControl, RangeControl, SelectControl, TextControl } from "./style-controls";

type LabelGeometry = Extract<MapGeometry, { kind: "label" }>;

/** Paths on the same map a label can follow, in creation order ("Path 1", "Path 2", ...). */
function followablePaths(objects: MapObject[]): { value: string; label: string }[] {
  return objects
    .filter((o) => o.geometry.kind === "path")
    .sort((a, b) => (a.createdAt < b.createdAt ? -1 : a.createdAt > b.createdAt ? 1 : 0))
    .map((o, i) => ({ value: o.id, label: `Path ${i + 1}` }));
}

/** Properties of a label: text, font, colour, rotation and "follow path". */
export function LabelEditor({ object, disabled, onStyleChange }: PropertyEditorProps) {
  const actions = useMapActions();
  const g = object.geometry;
  if (g.kind !== "label") return null;
  const look = labelLook(object.style);
  const follows = Boolean(g.pathId);
  const paths = followablePaths(actions.getObjects());

  function patchGeometry(label: string, changes: Partial<Omit<LabelGeometry, "kind">>) {
    actions.update(label, [{ id: object.id, patch: { geometry: { ...g, ...changes } } }]);
  }

  return (
    <div className="flex flex-col gap-2">
      <TextControl
        label="Text"
        value={g.text}
        disabled={disabled}
        maxLength={200}
        onCommit={(text) => patchGeometry("Edit label text", { text })}
      />
      <SelectControl
        label="Font"
        value={look.fontKey === "custom" ? "serif" : look.fontKey}
        options={LABEL_FONT_OPTIONS}
        disabled={disabled}
        onCommit={(fontFamily) => onStyleChange("Change label font", { fontFamily })}
      />
      <RangeControl
        label="Size"
        value={look.fontSize}
        min={8}
        max={200}
        step={1}
        disabled={disabled}
        format={(v) => `${v}px`}
        onCommit={(fontSize) => onStyleChange("Change label size", { fontSize })}
      />
      <RangeControl
        label="Letter spacing"
        value={look.letterSpacing}
        min={0}
        max={30}
        step={0.5}
        disabled={disabled}
        format={(v) => `${v}px`}
        onCommit={(letterSpacing) => onStyleChange("Change letter spacing", { letterSpacing })}
      />
      <ColorControl
        label="Colour"
        value={look.color}
        disabled={disabled}
        onCommit={(color) => onStyleChange("Change label colour", { color })}
      />
      {!follows && (
        <RangeControl
          label="Rotation"
          value={g.rotation}
          min={-180}
          max={180}
          step={5}
          disabled={disabled}
          format={(v) => `${v}°`}
          onCommit={(rotation) => patchGeometry("Rotate label", { rotation })}
        />
      )}
      {follows ? (
        <Button
          variant="outline"
          size="sm"
          disabled={disabled}
          onClick={() => patchGeometry("Detach label from path", { pathId: null })}
        >
          Detach from path
        </Button>
      ) : (
        paths.length > 0 && (
          <label className="flex items-center justify-between gap-2 text-xs">
            <span className="font-medium">Follow path</span>
            <select
              aria-label="Follow path"
              value=""
              disabled={disabled}
              onChange={(e) => {
                if (e.target.value) {
                  patchGeometry("Attach label to path", { pathId: e.target.value });
                }
              }}
              className="h-7 rounded-md border bg-background px-1.5 text-xs"
            >
              <option value="">Choose…</option>
              {paths.map((p) => (
                <option key={p.value} value={p.value}>
                  {p.label}
                </option>
              ))}
            </select>
          </label>
        )
      )}
    </div>
  );
}
