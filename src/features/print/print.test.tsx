import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { STANDARD } from "@/features/calendars/test-calendars";
import type { EntitySummary, EntityTypeDef } from "@/lib/bindings";
import { EntityPrintSection } from "./entity-print";
import { formatFieldValue } from "./format-field";
import { isLoreEmpty, renderLore } from "./lore-render";
import { capEntities, sortForPrint, tocGroups } from "./world-print";

const doc = (...content: unknown[]) => JSON.stringify({ type: "doc", content });
const text = (t: string, marks?: unknown[]) => ({ type: "text", text: t, marks });

const q = vi.hoisted(() => ({
  entity: {
    id: "e1",
    type: "custom-deity",
    name: "Aelith",
    summary: "Lady of the tides",
    tags: ["divine", "places/coast"],
    portrait: { assetId: "p", path: "C:/w/p.png", thumbPath: null },
    properties: { domain: "Sea", founded: 35, worshippers: ["f1", "gone"], stale: "x" },
  },
}));
vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: (p: string) => `asset://${p}` }));
vi.mock("@/features/entities/queries", () => ({
  useEntity: () => ({ isPending: false, data: q.entity }),
  useEntities: () => ({
    isPending: false,
    data: [{ id: "f1", type: "faction", name: "Tidewardens", updatedAt: "" }],
  }),
  useEntityTypes: () => ({
    isPending: false,
    data: [
      {
        key: "custom-deity",
        label: "Deity",
        plural: "Deities",
        icon: "sparkles",
        color: "#a855f7",
        builtin: false,
        fields: [
          { key: "domain", label: "Domain", kind: "select", options: ["Sea"], custom: true },
          { key: "founded", label: "Founded", kind: "date", options: [], custom: true },
          {
            key: "worshippers",
            label: "Worshippers",
            kind: { entityRef: { allowedTypes: [], multiple: true } },
            options: [],
            custom: true,
          },
          { key: "symbol", label: "Symbol", kind: "text", options: [], custom: true },
        ],
      },
      { key: "faction", label: "Faction", plural: "Factions", icon: "flag", color: "#dc2626" },
    ],
  }),
}));
vi.mock("@/features/gallery/queries", () => ({
  useEntityImages: () => ({
    isPending: false,
    data: [
      {
        id: "i1",
        fileName: "temple.png",
        caption: "The temple",
        path: "C:/w/temple.png",
        thumbPath: "C:/w/temple-thumb.png",
      },
    ],
  }),
}));
vi.mock("@/features/lore/queries", () => ({
  useDocument: () => ({
    isPending: false,
    data: {
      content: doc(
        { type: "heading", attrs: { level: 1 }, content: [text("Origin")] },
        {
          type: "paragraph",
          content: [text("Born of "), { type: "mention", attrs: { id: "f1" } }, text(" foam.")],
        },
      ),
    },
  }),
}));
vi.mock("@/features/relationships/queries", () => ({
  useRelationships: () => ({
    isPending: false,
    data: [
      {
        linkId: "l1",
        label: "sibling of",
        otherName: "Korrath",
        otherType: "custom-deity",
        otherId: "e2",
      },
    ],
  }),
}));
vi.mock("@/features/timeline/queries", () => ({
  useEventsForEntity: () => ({
    isPending: false,
    data: [{ entityId: "ev1", name: "The Great Flood", startDay: 40, endDay: null }],
  }),
  useEvent: () => ({ data: undefined }),
}));
vi.mock("@/features/calendars/queries", () => ({
  useDisplayCalendar: () => ({ isPending: false, data: STANDARD }),
}));
vi.mock("@/features/maps/queries", () => ({
  useMap: () => ({ isPending: false, data: undefined }),
}));

describe("EntityPrintSection", () => {
  it("prints fields, lore, relationships and events, and reports ready", () => {
    const onReady = vi.fn();
    render(<EntityPrintSection entityId="e1" onReady={onReady} />);

    expect(screen.getByRole("heading", { level: 1, name: "Aelith" })).toBeInTheDocument();
    expect(screen.getByText("Deity")).toBeInTheDocument();
    expect(screen.getByText("Lady of the tides")).toBeInTheDocument();

    // Fields: select value, date through the display calendar, refs by name (dangling marked).
    expect(screen.getByText("Domain").nextSibling).toHaveTextContent("Sea");
    expect(screen.getByText("Founded").nextSibling).toHaveTextContent(/35|Month|AF/);
    expect(screen.getByText("Worshippers").nextSibling).toHaveTextContent(
      "Tidewardens, missing entity",
    );
    // Empty field and unknown stored keys are not printed.
    expect(screen.queryByText("Symbol")).toBeNull();
    expect(screen.queryByText("stale")).toBeNull();

    // Lore: heading levels shift down one (the name is the h1), mentions print as names.
    expect(screen.getByRole("heading", { level: 2, name: "Origin" })).toBeInTheDocument();
    expect(screen.getByText("Tidewardens", { selector: ".mention" })).toBeInTheDocument();

    expect(screen.getByText("Relationships")).toBeInTheDocument();
    expect(screen.getByText("Korrath")).toBeInTheDocument();
    expect(screen.getByText("The Great Flood")).toBeInTheDocument();

    // Tags, portrait and gallery thumbnails.
    expect(screen.getByText("#divine #places/coast")).toBeInTheDocument();
    expect(document.querySelector("img.print-portrait")).toHaveAttribute(
      "src",
      "asset://C:/w/p.png",
    );
    expect(screen.getByRole("heading", { level: 2, name: "Gallery" })).toBeInTheDocument();
    expect(screen.getByAltText("The temple")).toHaveAttribute(
      "src",
      "asset://C:/w/temple-thumb.png",
    );
    expect(onReady).toHaveBeenCalledTimes(1);
  });
});

describe("renderLore", () => {
  it("renders marks, lists, tables and drops unsafe links", () => {
    const content = doc(
      {
        type: "paragraph",
        content: [
          text("bold", [{ type: "bold" }]),
          text("ok", [{ type: "link", attrs: { href: "https://example.com" } }]),
          text("bad", [{ type: "link", attrs: { href: "javascript:alert(1)" } }]),
        ],
      },
      {
        type: "bulletList",
        content: [{ type: "listItem", content: [{ type: "paragraph", content: [text("one")] }] }],
      },
      {
        type: "table",
        content: [{ type: "tableRow", content: [{ type: "tableCell", content: [text("cell")] }] }],
      },
    );
    const { container } = render(<div>{renderLore(content, () => undefined)}</div>);
    expect(container.querySelector("strong")).toHaveTextContent("bold");
    expect(container.querySelectorAll("a")).toHaveLength(1);
    expect(container.querySelector("a")).toHaveAttribute("href", "https://example.com");
    expect(container.querySelector("li")).toHaveTextContent("one");
    expect(container.querySelector("td")).toHaveTextContent("cell");
    expect(container).toHaveTextContent("bad");
  });

  it("survives unreadable content and marks missing mentions", () => {
    const { container } = render(<div>{renderLore("not json", () => undefined)}</div>);
    expect(container.firstChild).toBeEmptyDOMElement();
    const m = render(
      <div>{renderLore(doc({ type: "mention", attrs: { id: "x" } }), () => undefined)}</div>,
    );
    expect(m.container).toHaveTextContent("[missing entity]");
  });

  it("detects empty documents", () => {
    expect(isLoreEmpty(doc({ type: "paragraph" }))).toBe(true);
    expect(isLoreEmpty("garbage")).toBe(true);
    expect(isLoreEmpty(doc({ type: "paragraph", content: [text("hi")] }))).toBe(false);
  });
});

describe("formatFieldValue", () => {
  const ctx = { calendar: STANDARD.definition, nameOf: () => undefined };
  it("formats every kind and hides empty or misfit values", () => {
    expect(formatFieldValue({ kind: "boolean", options: [] }, true, ctx)).toBe("Yes");
    expect(formatFieldValue({ kind: "number", options: [] }, 3, ctx)).toBe("3");
    expect(formatFieldValue({ kind: "multiSelect", options: ["a", "b"] }, ["b", "a"], ctx)).toBe(
      "a, b",
    );
    expect(formatFieldValue({ kind: "select", options: ["a"] }, "gone", ctx)).toBeNull();
    expect(formatFieldValue({ kind: "text", options: [] }, "", ctx)).toBeNull();
    expect(formatFieldValue({ kind: "date", options: [] }, 0, ctx)).toMatch(/\S/);
  });
});

describe("world print helpers", () => {
  const types = [
    { key: "character", plural: "Characters" },
    { key: "location", plural: "Locations" },
  ] as EntityTypeDef[];
  const entities = [
    { id: "1", type: "location", name: "B" },
    { id: "2", type: "character", name: "Z" },
    { id: "3", type: "character", name: "A" },
  ] as EntitySummary[];

  it("sorts by type order then name, groups a table of contents and caps", () => {
    const sorted = sortForPrint(entities, types);
    expect(sorted.map((e) => e.id)).toEqual(["3", "2", "1"]);
    expect(tocGroups(sorted, types).map((g) => g.entities.length)).toEqual([2, 1]);
    expect(capEntities(sorted, 2)).toEqual({ shown: sorted.slice(0, 2), total: 3 });
  });
});
