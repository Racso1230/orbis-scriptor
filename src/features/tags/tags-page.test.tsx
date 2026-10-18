import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithQuery } from "@/test/render";
import { TagsPage } from "./tags-page";

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const commands = vi.hoisted(() => ({
  listTags: vi.fn(),
  listEntitiesByTag: vi.fn(),
  entityTypes: vi.fn(),
  renameTag: vi.fn(),
  deleteTag: vi.fn(),
  setTagColor: vi.fn(),
}));
vi.mock("@/lib/bindings", () => ({ commands }));
vi.mock("@/features/entities/use-open-entity", () => ({ useOpenEntity: () => vi.fn() }));

const TAGS = [
  { id: "t1", name: "coast", color: null, count: 2 },
  { id: "t2", name: "royalty", color: "#ff0000", count: 1 },
];

describe("TagsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    commands.listTags.mockResolvedValue(ok(TAGS));
    commands.entityTypes.mockResolvedValue(
      ok([
        {
          key: "character",
          label: "Character",
          plural: "Characters",
          icon: "user",
          color: "#2563eb",
          fields: [],
        },
      ]),
    );
    commands.listEntitiesByTag.mockResolvedValue(
      ok([
        {
          id: "e1",
          type: "character",
          name: "Isolde",
          updatedAt: "",
          favorite: false,
          tags: ["royalty"],
          portrait: null,
        },
      ]),
    );
    commands.renameTag.mockResolvedValue(ok({ id: "t1", name: "shore", color: null, count: 2 }));
    commands.deleteTag.mockResolvedValue(ok(null));
    commands.setTagColor.mockResolvedValue(ok({ ...TAGS[0], color: "#2563eb" }));
  });

  async function select(name: string) {
    fireEvent.click(await screen.findByRole("button", { name: new RegExp(`^${name}`) }));
  }

  it("lists tags with counts and shows the entities of the chosen tag", async () => {
    renderWithQuery(<TagsPage />);
    await select("royalty");
    expect(await screen.findByText("Isolde")).toBeInTheDocument();
    expect(commands.listEntitiesByTag).toHaveBeenCalledWith(["royalty"], false);
  });

  it("renames a tag", async () => {
    renderWithQuery(<TagsPage />);
    await select("coast");
    fireEvent.change(await screen.findByLabelText("Name"), { target: { value: "#shore" } });
    fireEvent.click(screen.getByRole("button", { name: "Rename" }));
    await waitFor(() => expect(commands.renameTag).toHaveBeenCalledWith("t1", "shore"));
  });

  it("warns that renaming to an existing tag merges, and offers Merge", async () => {
    renderWithQuery(<TagsPage />);
    await select("coast");
    fireEvent.change(await screen.findByLabelText("Name"), { target: { value: "Royalty" } });
    expect(screen.getByRole("status")).toHaveTextContent(/already exists.*merges/);
    fireEvent.click(screen.getByRole("button", { name: "Merge" }));
    await waitFor(() => expect(commands.renameTag).toHaveBeenCalledWith("t1", "Royalty"));
  });

  it("shows a validation message for a bad name without calling the backend", async () => {
    renderWithQuery(<TagsPage />);
    await select("coast");
    fireEvent.change(await screen.findByLabelText("Name"), { target: { value: "a//b" } });
    fireEvent.click(screen.getByRole("button", { name: "Rename" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/empty parts/);
    expect(commands.renameTag).not.toHaveBeenCalled();
  });

  it("saves and clears the colour", async () => {
    renderWithQuery(<TagsPage />);
    await select("royalty");
    fireEvent.click(await screen.findByRole("button", { name: "Save colour" }));
    await waitFor(() => expect(commands.setTagColor).toHaveBeenCalledWith("t2", "#ff0000"));
    fireEvent.click(screen.getByRole("button", { name: "Clear colour" }));
    await waitFor(() => expect(commands.setTagColor).toHaveBeenCalledWith("t2", null));
  });

  it("deletes a tag only after confirming", async () => {
    renderWithQuery(<TagsPage />);
    await select("coast");
    fireEvent.click(await screen.findByRole("button", { name: /Delete tag/ }));
    expect(commands.deleteTag).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole("button", { name: "Delete" }));
    await waitFor(() => expect(commands.deleteTag).toHaveBeenCalledWith("t1"));
  });
});
