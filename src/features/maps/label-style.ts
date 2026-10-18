import type { MapBackground, MapObject } from "@/lib/bindings";
import { isHexColor } from "./object-style";

type Style = MapObject["style"];

/** Font choices in the editor. The stored `fontFamily` is the key (or any CSS family list). */
export const LABEL_FONTS = {
  serif: { label: "Serif", css: 'Georgia, "Times New Roman", serif' },
  classic: {
    label: "Classic",
    css: '"Palatino Linotype", "Book Antiqua", Palatino, Georgia, serif',
  },
  sans: { label: "Sans-serif", css: '"Segoe UI", system-ui, Arial, sans-serif' },
} as const;
export type LabelFontKey = keyof typeof LABEL_FONTS;

export const LABEL_FONT_OPTIONS = (Object.keys(LABEL_FONTS) as LabelFontKey[]).map((value) => ({
  value,
  label: LABEL_FONTS[value].label,
}));

/** Ink colours for labels: dark brown on paper-like maps, near white on dark ones. */
export const LABEL_DARK = "#3b2a1a";
export const LABEL_LIGHT = "#f8fafc";

export interface LabelLook {
  fontSize: number;
  /** The key from `LABEL_FONTS` (or "custom" for an unknown stored value). */
  fontKey: LabelFontKey | "custom";
  /** CSS font-family list. */
  fontFamily: string;
  color: string;
  letterSpacing: number;
}

function isFontKey(value: unknown): value is LabelFontKey {
  return typeof value === "string" && value in LABEL_FONTS;
}

function clampNumber(value: unknown, fallback: number, min: number, max: number): number {
  return typeof value === "number" && Number.isFinite(value)
    ? Math.min(max, Math.max(min, value))
    : fallback;
}

/** Reads a label's style safely (the backend stores free-form JSON). */
export function labelLook(style: Style): LabelLook {
  const family = style["fontFamily"];
  const custom = typeof family === "string" && family.trim() !== "" && !isFontKey(family);
  return {
    fontSize: clampNumber(style["fontSize"], 24, 4, 400),
    fontKey: isFontKey(family) ? family : custom ? "custom" : "serif",
    fontFamily: isFontKey(family)
      ? LABEL_FONTS[family].css
      : custom
        ? family
        : LABEL_FONTS.serif.css,
    color: isHexColor(style["color"]) ? style["color"] : LABEL_DARK,
    letterSpacing: clampNumber(style["letterSpacing"], 0, 0, 100),
  };
}

/** Relative brightness 0..1 of a `#rrggbb` colour. */
export function luminance(hex: string): number {
  if (!isHexColor(hex)) return 0;
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
}

/** Whether a background is dark enough that labels should be light. */
function isDarkBackground(bg: MapBackground | null): boolean {
  if (!bg) return false; // image maps: assume a light, paper-like picture
  if (bg.kind === "ocean") return true;
  if (bg.kind === "plain") return bg.color !== null && luminance(bg.color) < 0.45;
  return false;
}

/**
 * The style a new label starts with: a size relative to the map (so a label is
 * readable on a 1000 px map and on an 8000 px one) and a colour that contrasts
 * with the background.
 */
export function defaultLabelStyle(
  mapWidth: number,
  background: MapBackground | null,
): { [key in string]: unknown } {
  return {
    fontSize: Math.max(16, Math.round(mapWidth / 60)),
    fontFamily: "serif",
    color: isDarkBackground(background) ? LABEL_LIGHT : LABEL_DARK,
  };
}

/** Outline ("halo") colour that keeps text of this colour readable on any background. */
export function haloColor(textColor: string): string {
  return luminance(textColor) < 0.5 ? "rgba(255, 250, 235, 0.75)" : "rgba(20, 15, 10, 0.75)";
}

let measureContext: CanvasRenderingContext2D | null | undefined;

/**
 * Width in pixels of one line of text. Uses a shared offscreen canvas; where
 * canvas is missing (unit tests) it falls back to a rough estimate.
 */
export function measureText(
  text: string,
  fontSize: number,
  fontFamily: string,
  letterSpacing = 0,
): number {
  if (measureContext === undefined) {
    try {
      measureContext =
        typeof document === "undefined" ? null : document.createElement("canvas").getContext("2d");
    } catch {
      measureContext = null;
    }
  }
  const extra = letterSpacing * Math.max(0, text.length - 1);
  if (!measureContext) return text.length * fontSize * 0.55 + extra;
  measureContext.font = `${fontSize}px ${fontFamily}`;
  return measureContext.measureText(text).width + extra;
}

/** The text a label may hold: one line, 1..200 characters. */
export function cleanLabelText(raw: string): string {
  return raw
    .replace(/[\r\n]+/g, " ")
    .trim()
    .slice(0, 200);
}
