import { describe, expect, it } from "vitest";
import {
  CUSTOM_SIZE,
  defaultParams,
  formatOffset,
  paramsProblem,
  parseSeed,
  parseSize,
  presetFor,
  randomSeed,
  rerollSeed,
  SEED_MAX,
} from "./wizard-model";

describe("wizard model", () => {
  it("defaults to a valid medium continent", () => {
    const p = defaultParams(7);
    expect(p).toMatchObject({ seed: 7, width: 4000, height: 3000, landShape: "continent" });
    expect(p.scope).toBe("mapOnly");
    expect(paramsProblem(p)).toBeNull();
  });

  it("makes random seeds in the u32 range", () => {
    expect(randomSeed(() => 0)).toBe(0);
    expect(randomSeed(() => 0.999999999999)).toBe(SEED_MAX);
    const s = randomSeed();
    expect(Number.isInteger(s) && s >= 0 && s <= SEED_MAX).toBe(true);
  });

  it("rerolls to a different seed", () => {
    expect(rerollSeed(0, () => 0)).toBe(1);
    expect(rerollSeed(SEED_MAX, () => 0.999999999999)).toBe(0);
    expect(rerollSeed(5, () => 0)).toBe(0);
  });

  it("maps sizes to presets", () => {
    expect(presetFor(2000, 1500)).toBe("small");
    expect(presetFor(6000, 4500)).toBe("large");
    expect(presetFor(1234, 1000)).toBe(CUSTOM_SIZE);
  });

  it("parses seeds and sizes strictly", () => {
    expect(parseSeed(" 42 ")).toBe(42);
    expect(parseSeed("4294967295")).toBe(SEED_MAX);
    expect(parseSeed("4294967296")).toBeNull();
    expect(parseSeed("-1")).toBeNull();
    expect(parseSeed("1.5")).toBeNull();
    expect(parseSeed("")).toBeNull();
    expect(parseSize("800")).toBe(800);
    expect(parseSize("8e2")).toBeNaN();
  });

  it("mirrors the Rust validation limits", () => {
    const ok = defaultParams(1);
    expect(paramsProblem({ ...ok, width: 255 })).toMatch(/Map size/);
    expect(paramsProblem({ ...ok, height: 16385 })).toMatch(/Map size/);
    expect(paramsProblem({ ...ok, width: Number.NaN })).toMatch(/Map size/);
    expect(paramsProblem({ ...ok, width: 256, height: 16384 })).toBeNull();
    expect(paramsProblem({ ...ok, landPercent: 9 })).toMatch(/Land/);
    expect(paramsProblem({ ...ok, landPercent: 91 })).toMatch(/Land/);
    expect(paramsProblem({ ...ok, kingdoms: 0 })).toMatch(/kingdoms/);
    expect(paramsProblem({ ...ok, kingdoms: 13 })).toMatch(/kingdoms/);
    expect(paramsProblem({ ...ok, temperature: 1.5 })).toMatch(/Temperature/);
    expect(paramsProblem({ ...ok, rainfall: -1.01 })).toMatch(/Rainfall/);
    expect(paramsProblem({ ...ok, seed: -1 })).toMatch(/Seed/);
    expect(paramsProblem({ ...ok, seed: SEED_MAX + 1 })).toMatch(/Seed/);
  });

  it("formats slider offsets", () => {
    expect(formatOffset(0)).toBe("0");
    expect(formatOffset(0.30000000000000004)).toBe("+0.3");
    expect(formatOffset(-1)).toBe("−1.0");
  });
});
