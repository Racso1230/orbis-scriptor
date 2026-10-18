import {
  DASH_PRESETS,
  areaStyle,
  dashFor,
  dashPresetOf,
  lineStyle,
  type DashPreset,
  type FillPattern,
} from "./object-style";
import { pathLength } from "./drawing";
import { ColorControl, RangeControl, SelectControl } from "./style-controls";
import type { PropertyEditorProps } from "./property-editor-types";

const DASH_OPTIONS = (Object.keys(DASH_PRESETS) as DashPreset[]).map((value) => ({
  value,
  label: DASH_PRESETS[value].label,
}));

const PATTERN_OPTIONS: readonly { value: FillPattern; label: string }[] = [
  { value: "none", label: "Solid" },
  { value: "hatch", label: "Hatched" },
  { value: "dots", label: "Dotted" },
];

/** Style fields for paths: colour, width, dash and smoothing. */
export function PathEditor({ object, disabled, onStyleChange }: PropertyEditorProps) {
  const s = lineStyle(object.style);
  const g = object.geometry;
  return (
    <div className="flex flex-col gap-2">
      {g.kind === "path" && (
        <p className="text-xs text-muted-foreground">
          {g.points.length} points, {Math.round(pathLength(g.points))} px long
        </p>
      )}
      <ColorControl
        label="Line colour"
        value={s.stroke}
        disabled={disabled}
        onCommit={(stroke) => onStyleChange("Change line colour", { stroke })}
      />
      <RangeControl
        label="Width"
        value={s.strokeWidth}
        min={1}
        max={40}
        step={1}
        disabled={disabled}
        format={(v) => `${v}px`}
        onCommit={(strokeWidth) =>
          onStyleChange("Change line width", {
            strokeWidth,
            // Keep dashes in proportion to the new width.
            ...(s.dash.length > 0 ? { dash: dashFor(dashPresetOf(s.dash), strokeWidth) } : {}),
          })
        }
      />
      <SelectControl
        label="Line style"
        value={dashPresetOf(s.dash)}
        options={DASH_OPTIONS}
        disabled={disabled}
        onCommit={(preset) =>
          onStyleChange("Change line style", { dash: dashFor(preset, s.strokeWidth) })
        }
      />
      <RangeControl
        label="Smoothing"
        value={s.smoothing}
        min={0}
        max={1}
        step={0.05}
        disabled={disabled}
        format={(v) => `${Math.round(v * 100)}%`}
        onCommit={(smoothing) => onStyleChange("Change smoothing", { smoothing })}
      />
    </div>
  );
}

/** Style fields for areas: fill colour, opacity, pattern and outline. */
export function AreaEditor({ object, disabled, onStyleChange }: PropertyEditorProps) {
  const s = areaStyle(object.style);
  return (
    <div className="flex flex-col gap-2">
      <ColorControl
        label="Fill colour"
        value={s.fill}
        disabled={disabled}
        // Moving the outline along keeps a "same colour" area looking intentional.
        onCommit={(fill) =>
          onStyleChange("Change fill colour", {
            fill,
            ...(s.stroke === s.fill ? { stroke: fill } : {}),
          })
        }
      />
      <RangeControl
        label="Fill opacity"
        value={s.fillOpacity}
        min={0}
        max={1}
        step={0.05}
        disabled={disabled}
        format={(v) => `${Math.round(v * 100)}%`}
        onCommit={(fillOpacity) => onStyleChange("Change fill opacity", { fillOpacity })}
      />
      <SelectControl
        label="Fill pattern"
        value={s.fillPattern}
        options={PATTERN_OPTIONS}
        disabled={disabled}
        onCommit={(fillPattern) => onStyleChange("Change fill pattern", { fillPattern })}
      />
      <ColorControl
        label="Outline colour"
        value={s.stroke}
        disabled={disabled}
        onCommit={(stroke) => onStyleChange("Change outline colour", { stroke })}
      />
      <RangeControl
        label="Outline width"
        value={s.strokeWidth}
        min={0}
        max={20}
        step={1}
        disabled={disabled}
        format={(v) => `${v}px`}
        onCommit={(strokeWidth) => onStyleChange("Change outline width", { strokeWidth })}
      />
      <SelectControl
        label="Outline style"
        value={dashPresetOf(s.dash)}
        options={DASH_OPTIONS}
        disabled={disabled}
        onCommit={(preset) =>
          onStyleChange("Change outline style", {
            dash: dashFor(preset, Math.max(2, s.strokeWidth)),
          })
        }
      />
    </div>
  );
}
