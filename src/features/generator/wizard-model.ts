import type { Climate, GeneratorParams, LandShape } from "@/lib/bindings";

// Keep in sync with `GeneratorParams::validate` and the map size limits in Rust.
export const MIN_MAP_SIZE = 256;
export const MAX_MAP_SIZE = 16_384;
export const LAND_MIN = 10;
export const LAND_MAX = 90;
export const KINGDOMS_MIN = 1;
export const KINGDOMS_MAX = 12;
/** Seeds are unsigned 32-bit integers. */
export const SEED_MAX = 4_294_967_295;
/** Longer side of the wizard's live preview, in pixels. */
export const PREVIEW_SIDE = 384;
/** Wait this long after the last change before rendering a new preview. */
export const PREVIEW_DEBOUNCE_MS = 400;

export interface SizePreset {
  id: string;
  label: string;
  width: number;
  height: number;
}

export const SIZE_PRESETS: SizePreset[] = [
  { id: "small", label: "Small (2000 × 1500)", width: 2000, height: 1500 },
  { id: "medium", label: "Medium (4000 × 3000)", width: 4000, height: 3000 },
  { id: "large", label: "Large (6000 × 4500)", width: 6000, height: 4500 },
];
export const CUSTOM_SIZE = "custom";

export const LAND_SHAPES: { value: LandShape; label: string }[] = [
  { value: "continent", label: "Continent" },
  { value: "twinContinents", label: "Twin continents" },
  { value: "archipelago", label: "Archipelago" },
  { value: "pangaea", label: "Pangaea" },
  { value: "coastline", label: "Coastline" },
];

export const CLIMATES: { value: Climate; label: string }[] = [
  { value: "temperate", label: "Temperate" },
  { value: "frozen", label: "Frozen" },
  { value: "arid", label: "Arid" },
  { value: "tropical", label: "Tropical" },
  { value: "volcanic", label: "Volcanic" },
];

/** A random seed in 0..=SEED_MAX. `random` is injectable for tests. */
export function randomSeed(random: () => number = Math.random): number {
  return Math.floor(random() * (SEED_MAX + 1)) % (SEED_MAX + 1);
}

/** A random seed that differs from `current` (so "Reroll" always changes the world). */
export function rerollSeed(current: number, random: () => number = Math.random): number {
  const next = randomSeed(random);
  return next === current ? (next + 1) % (SEED_MAX + 1) : next;
}

/** The wizard's starting settings: a medium continent with a random seed. */
export function defaultParams(seed: number = randomSeed()): GeneratorParams {
  return {
    seed,
    width: 4000,
    height: 3000,
    landShape: "continent",
    climate: "temperate",
    landPercent: 40,
    temperature: 0,
    rainfall: 0,
    kingdoms: 5,
    scope: "mapOnly",
  };
}

/** The preset matching a size, or `CUSTOM_SIZE`. */
export function presetFor(width: number, height: number): string {
  return SIZE_PRESETS.find((p) => p.width === width && p.height === height)?.id ?? CUSTOM_SIZE;
}

/** The seed typed by the user, or `null` when it is not a whole number in range. */
export function parseSeed(text: string): number | null {
  const trimmed = text.trim();
  if (!/^\d+$/.test(trimmed)) return null;
  const value = Number(trimmed);
  return value <= SEED_MAX ? value : null;
}

/** A whole number typed into a size field, or `NaN`. */
export function parseSize(text: string): number {
  const trimmed = text.trim();
  return /^\d+$/.test(trimmed) ? Number(trimmed) : Number.NaN;
}

function inRange(value: number, min: number, max: number): boolean {
  return Number.isFinite(value) && value >= min && value <= max;
}

/** The first problem with `params` in plain words (same rules as Rust), or `null`. */
export function paramsProblem(params: GeneratorParams): string | null {
  if (!Number.isInteger(params.seed) || !inRange(params.seed, 0, SEED_MAX)) {
    return `Seed must be a whole number from 0 to ${SEED_MAX}`;
  }
  const sizeOk = (v: number) => Number.isInteger(v) && inRange(v, MIN_MAP_SIZE, MAX_MAP_SIZE);
  if (!sizeOk(params.width) || !sizeOk(params.height)) {
    return `Map size must be between ${MIN_MAP_SIZE} and ${MAX_MAP_SIZE} pixels per side`;
  }
  if (!inRange(params.landPercent, LAND_MIN, LAND_MAX)) {
    return `Land must be between ${LAND_MIN} and ${LAND_MAX} percent`;
  }
  if (!inRange(params.kingdoms, KINGDOMS_MIN, KINGDOMS_MAX)) {
    return `Number of kingdoms must be between ${KINGDOMS_MIN} and ${KINGDOMS_MAX}`;
  }
  if (!inRange(params.temperature, -1, 1)) return "Temperature must be between -1 and 1";
  if (!inRange(params.rainfall, -1, 1)) return "Rainfall must be between -1 and 1";
  return null;
}

/** Slider value as text: "−0.3", "0", "+0.5". */
export function formatOffset(value: number): string {
  const rounded = Math.round(value * 10) / 10;
  if (rounded === 0) return "0";
  return rounded > 0 ? `+${rounded.toFixed(1)}` : `−${Math.abs(rounded).toFixed(1)}`;
}
