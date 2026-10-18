import type { MapObject } from "@/lib/bindings";

/** Styles are free-form JSON on the backend; these helpers read them safely. */
type Style = MapObject["style"];

export type PathPreset = "river" | "road" | "border" | "freehand";
export type FillPattern = "none" | "hatch" | "dots";

export interface PresetInfo {
  label: string;
  style: { [key in string]: unknown };
}

/** Starting styles for the Path tool presets (and Freehand). */
export const PATH_PRESETS: Record<PathPreset, PresetInfo> = {
  river: { label: "River", style: { stroke: "#3b82f6", strokeWidth: 6, smoothing: 0.5 } },
  road: {
    label: "Road",
    style: { stroke: "#8b5a2b", strokeWidth: 4, dash: [12, 8], smoothing: 0.2 },
  },
  border: {
    label: "Border",
    style: { stroke: "#3f3f46", strokeWidth: 3, dash: [14, 6], smoothing: 0 },
  },
  freehand: { label: "Freehand", style: { stroke: "#334155", strokeWidth: 4, smoothing: 0.4 } },
};

/** Style for a new area: a colour with a light (25%) fill. */
export const DEFAULT_AREA_STYLE: { [key in string]: unknown } = {
  stroke: "#7c3aed",
  strokeWidth: 3,
  fill: "#7c3aed",
  fillOpacity: 0.25,
  fillPattern: "none",
};

/** Dash presets in the style editor (lengths scale with the line width). */
export const DASH_PRESETS = {
  solid: { label: "Solid", pattern: [] as number[] },
  dashed: { label: "Dashed", pattern: [3, 2] },
  dotted: { label: "Dotted", pattern: [0.4, 2] },
} as const;
export type DashPreset = keyof typeof DASH_PRESETS;

const HEX = /^#[0-9a-fA-F]{6}$/;

export function isHexColor(value: unknown): value is string {
  return typeof value === "string" && HEX.test(value);
}

function num(style: Style, key: string, fallback: number, min: number, max: number): number {
  const v = style[key];
  return typeof v === "number" && Number.isFinite(v) ? Math.min(max, Math.max(min, v)) : fallback;
}

function colour(style: Style, key: string, fallback: string): string {
  const v = style[key];
  return isHexColor(v) ? v : fallback;
}

export interface LineStyle {
  stroke: string;
  strokeWidth: number;
  dash: number[];
  /** Konva line tension; 0 = straight segments. */
  smoothing: number;
}

export interface AreaStyle extends LineStyle {
  fill: string;
  fillOpacity: number;
  fillPattern: FillPattern;
}

function readDash(style: Style): number[] {
  const v = style["dash"];
  if (!Array.isArray(v)) return [];
  const nums = v.filter((n): n is number => typeof n === "number" && Number.isFinite(n) && n >= 0);
  // Konva needs a positive total or it can loop forever drawing the dash.
  return nums.length > 0 && nums.reduce((a, b) => a + b, 0) > 0 ? nums : [];
}

export function lineStyle(style: Style): LineStyle {
  return {
    stroke: colour(style, "stroke", "#334155"),
    strokeWidth: num(style, "strokeWidth", 4, 0.5, 200),
    dash: readDash(style),
    smoothing: num(style, "smoothing", 0, 0, 1),
  };
}

export function areaStyle(style: Style): AreaStyle {
  const fill = colour(style, "fill", DEFAULT_AREA_STYLE["fill"] as string);
  const pattern = style["fillPattern"];
  return {
    stroke: colour(style, "stroke", fill),
    strokeWidth: num(style, "strokeWidth", 2, 0, 200),
    dash: readDash(style),
    smoothing: num(style, "smoothing", 0, 0, 1),
    fill,
    fillOpacity: num(style, "fillOpacity", 0.25, 0, 1),
    fillPattern: pattern === "hatch" || pattern === "dots" ? pattern : "none",
  };
}

/** Dash array for a preset at a given line width (what the editor stores). */
export function dashFor(preset: DashPreset, strokeWidth: number): number[] {
  return DASH_PRESETS[preset].pattern.map((n) => Math.max(0.1, n * Math.max(2, strokeWidth)));
}

/** Which preset a stored dash array best matches (for showing the select). */
export function dashPresetOf(dash: number[]): DashPreset {
  if (dash.length === 0) return "solid";
  const first = dash[0] ?? 0;
  const second = dash[1] ?? first;
  return first < second / 2 ? "dotted" : "dashed";
}

/** `#rrggbb` + opacity -> `rgba(r, g, b, a)` (Konva has no separate fill opacity). */
export function withOpacity(hex: string, opacity: number): string {
  if (!isHexColor(hex)) return `rgba(0, 0, 0, ${opacity})`;
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgba(${r}, ${g}, ${b}, ${opacity})`;
}

/** Colour used to draw the selection halo around any object. */
export const SELECTION_COLOR = "#facc15";
