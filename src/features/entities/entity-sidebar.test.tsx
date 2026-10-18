import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { EntitySummary } from "@/lib/bindings";
import { renderWithQuery } from "@/test/render";
import { EntitySidebar } from "./entity-sidebar";

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const commands = vi.hoisted(() => ({
  entityTypes: vi.fn(),
  listEntities: vi.fn(),
  listFavorites: vi.fn(),
  listRecent: vi.fn(),
  listTags: vi.fn(),
  setFavorite: vi.fn(),
  createEntity: vi.fn(),
}));
vi.mock("@/lib/bindings", () => ({ commands }));
vi.mock("@/features/worlds/queries", () => ({ useCurrentWorld: () => ({ data: { path: "w" } }) }));
vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => vi.fn(),
  useParams: () => ({ id: undefined }),
  useRouterState: () => false,
}));
// jsdom has no layout, so the real virtualiser would render no rows at all.
vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    getTotalSize: () => count * 32,
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({
        index,
        key: index,
        size: 32,
        start: index * 32,
      })),
  }),
}));

const entity = (id: string, name: string, over: Partial<EntitySummary> = {}): EntitySummary => ({
  id,
  type: "character",
  name,
  updatedAt: new Date().toISOString(),
  favorite: false,
  tags: [],
  portrait: null,
  ...over,
});
const ISOLDE = entity("1", "Isolde", { favorite: true, tags: ["royalty"] });
const BRAN = entity("2", "Bran", { tags: ["coast", "places/wilds"] });
const CEDRIC = entity("3", "Cedric", {
  portrait: { assetId: "a", path: "C:/w/a.png", thumbPath: "C:/w/a-thumb.png" },
});

describe("EntitySidebar", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    commands.entityTypes.mockResolvedValue(
      ok([
        {
          key: "character",
          label: "Character",
          plural: "Characters",
          icon: "user",
          color: "#2563eb",
          builtin: true,
          fields: [],
        },
      ]),
    );
    commands.listEntities.mockResolvedValue(ok([BRAN, CEDRIC, ISOLDE]));
    commands.listFavorites.mockResolvedValue(ok([ISOLDE]));
    commands.listRecent.mockResolvedValue(ok([CEDRIC, ISOLDE]));
    commands.listTags.mockResolvedValue(
      ok([
        { id: "t1", name: "coast", color: null, count: 1 },
        { id: "t2", name: "royalty", color: "#ff0000", count: 1 },
        { id: "t3", name: "places/wilds", color: null, count: 1 },
      ]),
    );
    commands.setFavorite.mockResolvedValue(ok(null));
  });

  it("shows Favourites and Recently edited above the type groups", async () => {
    renderWithQuery(<EntitySidebar />);
    expect(await screen.findByRole("button", { name: /Favourites/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Recently edited/ })).toBeInTheDocument();
    expect(screen.getAllByText("just now")).toHaveLength(2);
    expect(commands.listRecent).toHaveBeenCalledWith(8);
  });

  it("collapses a section", async () => {
    renderWithQuery(<EntitySidebar />);
    const header = await screen.findByRole("button", { name: /Favourites/ });
    expect(document.querySelectorAll('[data-section="favorites"]')).toHaveLength(1);
    fireEvent.click(header);
    expect(document.querySelectorAll('[data-section="favorites"]')).toHaveLength(0);
  });

  it("toggles the favourite star", async () => {
    renderWithQuery(<EntitySidebar />);
    await screen.findByRole("button", { name: /Favourites/ });
    const favRow = document.querySelector('[data-section="favorites"]') as HTMLElement;
    fireEvent.click(within(favRow).getByRole("button", { name: "Remove from favourites" }));
    await waitFor(() => expect(commands.setFavorite).toHaveBeenCalledWith("1", false));

    const rows = document.querySelectorAll('[data-section="recent"]');
    const cedricRow = rows[0] as HTMLElement;
    fireEvent.click(within(cedricRow).getByRole("button", { name: "Add to favourites" }));
    await waitFor(() => expect(commands.setFavorite).toHaveBeenCalledWith("3", true));
  });

  it("shows a portrait as the avatar instead of the type icon", async () => {
    renderWithQuery(<EntitySidebar />);
    await screen.findByRole("button", { name: /Characters/ });
    const avatars = await screen.findAllByTestId("entity-avatar");
    expect(avatars.length).toBeGreaterThan(0);
    expect(avatars[0]).toHaveAttribute("src", expect.stringContaining("a-thumb.png"));
  });

  it("filters by tag (any / all, nested) and hides the top sections while filtering", async () => {
    renderWithQuery(<EntitySidebar />);
    await screen.findByRole("button", { name: /Favourites/ });
    fireEvent.click(await screen.findByRole("button", { name: /Filter by tag/ }));

    const chips = screen.getByRole("group", { name: "Tags" });
    fireEvent.click(within(chips).getByRole("button", { name: "royalty" }));
    // Any: only Isolde has royalty.
    await waitFor(() => expect(screen.queryByRole("button", { name: /Favourites/ })).toBeNull());
    const list = () => screen.getAllByRole("button").map((b) => b.textContent);
    expect(list()).toContain("Isolde");
    expect(list()).not.toContain("Bran");

    fireEvent.click(within(chips).getByRole("button", { name: "royalty" }));
    fireEvent.click(within(chips).getByRole("button", { name: "coast" }));
    fireEvent.click(within(chips).getByRole("button", { name: "places/wilds" }));
    fireEvent.click(screen.getByRole("button", { name: "All" }));
    // All of coast AND places/wilds: Bran only.
    await waitFor(() => expect(list()).not.toContain("Isolde"));
    expect(list()).toContain("Bran");

    fireEvent.click(screen.getByRole("button", { name: "Clear tag filter" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /Favourites/ })).toBeInTheDocument(),
    );
  });
});
