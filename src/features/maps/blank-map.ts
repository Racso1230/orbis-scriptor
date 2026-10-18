import type { MapBackground } from "@/lib/bindings";

export const MIN_MAP_SIZE = 256;
export const MAX_MAP_SIZE = 16384;

export const SIZE_PRESETS = [
  { id: "small", label: "2048 × 1536", width: 2048, height: 1536 },
  { id: "large", label: "4096 × 3072", width: 4096, height: 3072 },
] as const;

export interface BlankMapForm {
  /** Preset id or "custom". */
  size: string;
  /** Raw text of the custom inputs. */
  customWidth: string;
  customHeight: string;
  background: MapBackground;
}

export interface ValidBlankMap {
  width: number;
  height: number;
  background: MapBackground;
}

export type BlankMapResult =
  { ok: true; value: ValidBlankMap } | { ok: false; errors: { size?: string; color?: string } };

function parseDimension(text: string): number | null {
  if (!/^\d+$/.test(text.trim())) return null;
  return Number(text.trim());
}

/** Checks the dialog's fields; returns the values to send or per-field errors. */
export function validateBlankMap(form: BlankMapForm): BlankMapResult {
  const errors: { size?: string; color?: string } = {};
  let width = 0;
  let height = 0;

  const preset = SIZE_PRESETS.find((p) => p.id === form.size);
  if (preset) {
    width = preset.width;
    height = preset.height;
  } else {
    const w = parseDimension(form.customWidth);
    const h = parseDimension(form.customHeight);
    if (w === null || h === null) {
      errors.size = "Enter whole numbers for width and height.";
    } else if (w < MIN_MAP_SIZE || w > MAX_MAP_SIZE || h < MIN_MAP_SIZE || h > MAX_MAP_SIZE) {
      errors.size = `Each side must be between ${MIN_MAP_SIZE} and ${MAX_MAP_SIZE} pixels.`;
    } else {
      width = w;
      height = h;
    }
  }

  const { color, kind } = form.background;
  if (color !== null && !/^#[0-9a-fA-F]{6}$/.test(color)) {
    errors.color = "Use a colour like #a1b2c3.";
  } else if (kind === "plain" && color === null) {
    errors.color = "Pick a colour for a plain map.";
  }

  if (errors.size || errors.color) return { ok: false, errors };
  return { ok: true, value: { width, height, background: form.background } };
}
