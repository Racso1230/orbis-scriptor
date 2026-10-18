import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { EntityIcon, TYPE_ICONS } from "./entity-icon";
import { DEFAULT_TYPE_COLOR, lookFor } from "./type-look";

// Copy of the backend allow-list (services/entity_types.rs `type_icons`).
export const BACKEND_ICONS = [
  "user", "users", "crown", "skull", "ghost", "flag", "sword", "swords", "shield", "castle",
  "tent", "church", "landmark", "map-pin", "map", "compass", "globe", "anchor", "ship", "mountain",
  "trees", "tree-pine", "flame", "droplets", "wind", "snowflake", "sun", "moon", "star", "sparkles",
  "book-open", "scroll", "bookmark", "sticky-note", "calendar-days", "gem", "coins", "key",
  "hammer", "flask-conical", "package", "box", "heart", "eye", "feather", "bird", "cat", "dog",
  "fish", "bug",
]; // prettier-ignore

describe("icon map", () => {
  it("covers every allow-listed icon name", () => {
    expect(BACKEND_ICONS).toHaveLength(50);
    for (const name of BACKEND_ICONS) expect(TYPE_ICONS[name], name).toBeDefined();
  });

  it("has no extra names the backend would refuse", () => {
    expect(Object.keys(TYPE_ICONS).sort()).toEqual([...BACKEND_ICONS].sort());
  });

  it("falls back to the box icon for unknown names", () => {
    const { container: unknown } = render(<EntityIcon icon="nope" />);
    const { container: box } = render(<EntityIcon icon="box" />);
    expect(unknown.innerHTML).toBe(box.innerHTML);
  });
});

describe("lookFor", () => {
  it("uses the type colour and the first letter of the label", () => {
    expect(lookFor({ color: "#a855f7", label: "deity" })).toEqual({
      color: "#a855f7",
      glyph: "D",
    });
  });

  it("falls back for unknown types and malformed colours", () => {
    expect(lookFor(undefined)).toEqual({ color: DEFAULT_TYPE_COLOR, glyph: "•" });
    expect(lookFor({ color: "red", label: "" }).color).toBe(DEFAULT_TYPE_COLOR);
    expect(lookFor({ color: "red", label: "" }).glyph).toBe("•");
  });
});
