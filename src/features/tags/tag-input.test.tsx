import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TagInput } from "./tag-input";

const KNOWN = ["coast", "royalty", "places/cities"];

function setup(value: string[] = ["coast"], onChange = vi.fn().mockResolvedValue(undefined)) {
  render(<TagInput value={value} known={KNOWN} onChange={onChange} />);
  return { onChange, input: screen.getByLabelText("Add tag") as HTMLInputElement };
}

describe("TagInput", () => {
  it("adds a tag on Enter and strips a leading #", async () => {
    const { onChange, input } = setup();
    fireEvent.change(input, { target: { value: "#Dragons" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(onChange).toHaveBeenCalledWith(["coast", "Dragons"]));
    await waitFor(() => expect(input.value).toBe(""));
  });

  it("adds a tag when a comma is typed", async () => {
    const { onChange, input } = setup([]);
    fireEvent.change(input, { target: { value: "high king" } });
    fireEvent.keyDown(input, { key: "," });
    await waitFor(() => expect(onChange).toHaveBeenCalledWith(["high king"]));
  });

  it("adds several tags from pasted text in one save", async () => {
    const { onChange, input } = setup([]);
    fireEvent.change(input, { target: { value: "#a, b ,#c" } });
    await waitFor(() => expect(onChange).toHaveBeenCalledWith(["a", "b", "c"]));
  });

  it("removes a tag with its x button", async () => {
    const { onChange } = setup(["coast", "royalty"]);
    fireEvent.click(screen.getByRole("button", { name: "Remove tag coast" }));
    await waitFor(() => expect(onChange).toHaveBeenCalledWith(["royalty"]));
  });

  it("removes the last tag with Backspace on an empty field", async () => {
    const { onChange, input } = setup(["coast", "royalty"]);
    fireEvent.keyDown(input, { key: "Backspace" });
    await waitFor(() => expect(onChange).toHaveBeenCalledWith(["coast"]));
  });

  it("does not add the same tag twice (ignoring case) and reuses the known spelling", async () => {
    const { onChange, input } = setup(["coast"]);
    fireEvent.change(input, { target: { value: "COAST" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(input.value).toBe(""));
    expect(onChange).not.toHaveBeenCalled();

    fireEvent.change(input, { target: { value: "ROYALTY" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(onChange).toHaveBeenCalledWith(["coast", "royalty"]));
  });

  it("suggests known tags and adds one on click", async () => {
    const { onChange, input } = setup(["coast"]);
    fireEvent.change(input, { target: { value: "ro" } });
    const option = await screen.findByRole("button", { name: "royalty" });
    expect(screen.queryByRole("button", { name: "coast" })).toBeNull();
    fireEvent.click(option);
    await waitFor(() => expect(onChange).toHaveBeenCalledWith(["coast", "royalty"]));
  });

  it("shows a validation error for bad names without saving", async () => {
    const { onChange, input } = setup([]);
    fireEvent.change(input, { target: { value: "a//b" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(await screen.findByRole("alert")).toHaveTextContent(/empty parts/);
    expect(onChange).not.toHaveBeenCalled();

    fireEvent.change(input, { target: { value: "bad!" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(await screen.findByRole("alert")).toHaveTextContent(/letters, digits/);
  });

  it("shows the message when saving fails and keeps the typed text", async () => {
    const onChange = vi.fn().mockRejectedValue(new Error("At most 50 tags"));
    const { input } = setup([], onChange);
    fireEvent.change(input, { target: { value: "extra" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(await screen.findByRole("alert")).toHaveTextContent("At most 50 tags");
    expect(input.value).toBe("extra");
  });
});
