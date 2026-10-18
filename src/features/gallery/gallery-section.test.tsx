import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";
import type { EntityImage } from "@/lib/bindings";
import { renderWithQuery } from "@/test/render";
import { GallerySection } from "./gallery-section";
import { Lightbox } from "./lightbox";

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const commands = vi.hoisted(() => ({
  addEntityImage: vi.fn(),
  updateEntityImage: vi.fn(),
  reorderEntityImages: vi.fn(),
  removeEntityImage: vi.fn(),
  setPortrait: vi.fn(),
}));
const pick = vi.hoisted(() => vi.fn());
vi.mock("@/lib/bindings", () => ({ commands }));
vi.mock("./pick-images", () => ({ pickImageFiles: pick }));

const image = (id: string, over: Partial<EntityImage> = {}): EntityImage => ({
  id,
  entityId: "e1",
  assetId: `asset-${id}`,
  caption: "",
  sortOrder: 0,
  isPortrait: false,
  createdAt: "",
  fileName: `${id}.png`,
  width: 10,
  height: 10,
  path: `C:/w/assets/${id}.png`,
  thumbPath: `C:/w/assets/${id}-thumb.png`,
  ...over,
});
const IMAGES = [image("a"), image("b", { caption: "Second" }), image("c", { isPortrait: true })];

function setup(images = IMAGES) {
  const refetch = vi.fn();
  const onOpen = vi.fn();
  renderWithQuery(
    <GallerySection entityId="e1" images={images} refetch={refetch} onOpen={onOpen} />,
  );
  return { refetch, onOpen };
}

describe("GallerySection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    commands.addEntityImage.mockResolvedValue(ok(image("new")));
    commands.updateEntityImage.mockResolvedValue(ok(image("b")));
    commands.reorderEntityImages.mockResolvedValue(ok(IMAGES));
    commands.removeEntityImage.mockResolvedValue(ok(null));
    commands.setPortrait.mockResolvedValue(ok(IMAGES));
  });

  it("adds the picked images one after the other", async () => {
    pick.mockResolvedValue(["C:/x/1.png", "C:/x/2.png"]);
    setup();
    fireEvent.click(screen.getByRole("button", { name: /Add images/ }));
    await waitFor(() => expect(commands.addEntityImage).toHaveBeenCalledTimes(2));
    expect(commands.addEntityImage).toHaveBeenNthCalledWith(1, "e1", "C:/x/1.png");
    expect(commands.addEntityImage).toHaveBeenNthCalledWith(2, "e1", "C:/x/2.png");
  });

  it("adds nothing when the picker is cancelled", async () => {
    pick.mockResolvedValue([]);
    setup();
    fireEvent.click(screen.getByRole("button", { name: /Add images/ }));
    await waitFor(() => expect(pick).toHaveBeenCalled());
    expect(commands.addEntityImage).not.toHaveBeenCalled();
  });

  it("shows the error when adding fails", async () => {
    pick.mockResolvedValue(["C:/x/1.png"]);
    commands.addEntityImage.mockResolvedValue({
      status: "error",
      error: { kind: "validation", message: "Not an image" },
    });
    setup();
    fireEvent.click(screen.getByRole("button", { name: /Add images/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Not an image");
  });

  it("reorders with the earlier / later buttons (all ids, once each)", async () => {
    setup();
    fireEvent.click(screen.getByRole("button", { name: "Move b.png earlier" }));
    await waitFor(() =>
      expect(commands.reorderEntityImages).toHaveBeenCalledWith("e1", ["b", "a", "c"]),
    );
    fireEvent.click(screen.getByRole("button", { name: "Move b.png later" }));
    await waitFor(() =>
      expect(commands.reorderEntityImages).toHaveBeenLastCalledWith("e1", ["a", "c", "b"]),
    );
    expect(screen.getByRole("button", { name: "Move a.png earlier" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Move c.png later" })).toBeDisabled();
  });

  it("saves a changed caption on Enter and on blur, but not an unchanged one", async () => {
    setup();
    const field = screen.getByLabelText("Caption of a.png");
    fireEvent.change(field, { target: { value: "A castle" } });
    fireEvent.keyDown(field, { key: "Enter" });
    await waitFor(() => expect(commands.updateEntityImage).toHaveBeenCalledWith("a", "A castle"));

    commands.updateEntityImage.mockClear();
    const second = screen.getByLabelText("Caption of b.png");
    fireEvent.blur(second);
    expect(commands.updateEntityImage).not.toHaveBeenCalled();
    fireEvent.change(second, { target: { value: "" } });
    fireEvent.blur(second);
    await waitFor(() => expect(commands.updateEntityImage).toHaveBeenCalledWith("b", ""));
  });

  it("sets and clears the portrait", async () => {
    setup();
    fireEvent.click(screen.getAllByRole("button", { name: "Set as portrait" })[0]!);
    await waitFor(() => expect(commands.setPortrait).toHaveBeenCalledWith("e1", "a"));
    fireEvent.click(screen.getByRole("button", { name: "Remove portrait" }));
    await waitFor(() => expect(commands.setPortrait).toHaveBeenLastCalledWith("e1", null));
    expect(screen.getByText("Portrait")).toBeInTheDocument();
  });

  it("removes an image only after confirming", async () => {
    setup();
    fireEvent.click(screen.getByRole("button", { name: "Remove a.png" }));
    expect(commands.removeEntityImage).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole("button", { name: "Remove" }));
    await waitFor(() => expect(commands.removeEntityImage).toHaveBeenCalledWith("a"));
  });

  it("opens the lightbox at the clicked image", () => {
    const { onOpen } = setup();
    fireEvent.click(screen.getByRole("button", { name: "Open Second" }));
    expect(onOpen).toHaveBeenCalledWith(1);
  });

  it("shows an empty state", () => {
    setup([]);
    expect(screen.getByText(/No images yet/)).toBeInTheDocument();
  });
});

describe("GallerySection thumbnails", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("asks again for images whose thumbnail is still being made, a limited number of times", () => {
    const { refetch } = setup([image("a", { thumbPath: null })]);
    act(() => {
      vi.advanceTimersByTime(1500);
    });
    expect(refetch).toHaveBeenCalledTimes(1);
  });

  it("does not poll when every thumbnail exists", () => {
    const { refetch } = setup();
    act(() => {
      vi.advanceTimersByTime(10_000);
    });
    expect(refetch).not.toHaveBeenCalled();
  });
});

describe("Lightbox", () => {
  it("shows the caption and steps with the buttons and arrow keys (wrapping)", () => {
    const onIndexChange = vi.fn();
    const onClose = vi.fn();
    renderWithQuery(
      <Lightbox images={IMAGES} index={1} onIndexChange={onIndexChange} onClose={onClose} />,
    );
    expect(screen.getByText("Second", { selector: "p" })).toBeInTheDocument();
    expect(screen.getByText("2 / 3")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Next image" }));
    expect(onIndexChange).toHaveBeenLastCalledWith(2);
    fireEvent.click(screen.getByRole("button", { name: "Previous image" }));
    expect(onIndexChange).toHaveBeenLastCalledWith(0);
    fireEvent.keyDown(window, { key: "ArrowRight" });
    expect(onIndexChange).toHaveBeenLastCalledWith(2);
  });

  it("wraps from the last image to the first and closes on Escape", () => {
    const onIndexChange = vi.fn();
    const onClose = vi.fn();
    renderWithQuery(
      <Lightbox images={IMAGES} index={2} onIndexChange={onIndexChange} onClose={onClose} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Next image" }));
    expect(onIndexChange).toHaveBeenLastCalledWith(0);
    fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
  });

  it("renders nothing when closed", () => {
    renderWithQuery(
      <Lightbox images={IMAGES} index={null} onIndexChange={vi.fn()} onClose={vi.fn()} />,
    );
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
