import { describe, expect, it } from "vitest";
import { formatRelativeTime } from "./relative-time";

const NOW = Date.parse("2026-05-20T12:00:00Z");
const ago = (ms: number) => new Date(NOW - ms).toISOString();
const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;

describe("formatRelativeTime", () => {
  it("says just now for the last minute (and for times slightly in the future)", () => {
    expect(formatRelativeTime(ago(5_000), NOW)).toBe("just now");
    expect(formatRelativeTime(ago(-5_000), NOW)).toBe("just now");
  });

  it("uses minutes, hours, yesterday, days and weeks", () => {
    expect(formatRelativeTime(ago(5 * MIN), NOW)).toBe("5 min ago");
    expect(formatRelativeTime(ago(3 * HOUR + 10 * MIN), NOW)).toBe("3 h ago");
    expect(formatRelativeTime(ago(30 * HOUR), NOW)).toBe("yesterday");
    expect(formatRelativeTime(ago(4 * DAY), NOW)).toBe("4 d ago");
    expect(formatRelativeTime(ago(15 * DAY), NOW)).toBe("2 w ago");
  });

  it("falls back to a date for old timestamps", () => {
    expect(formatRelativeTime(ago(200 * DAY), NOW)).toMatch(/2025/);
  });

  it("returns an empty string for text that is not a date", () => {
    expect(formatRelativeTime("", NOW)).toBe("");
    expect(formatRelativeTime("nonsense", NOW)).toBe("");
  });
});
