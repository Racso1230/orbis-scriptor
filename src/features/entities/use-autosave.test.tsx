import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAutosave } from "./use-autosave";

describe("useAutosave", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("saves once after the value stops changing", async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    const { result, rerender } = renderHook(({ v }) => useAutosave(v, save, 500), {
      initialProps: { v: { name: "a" } },
    });
    expect(result.current.status).toBe("saved");

    rerender({ v: { name: "ab" } });
    expect(result.current.status).toBe("dirty");
    await act(() => vi.advanceTimersByTimeAsync(300));
    rerender({ v: { name: "abc" } });
    await act(() => vi.advanceTimersByTimeAsync(300));
    expect(save).not.toHaveBeenCalled(); // timer was reset by the second edit

    await act(() => vi.advanceTimersByTimeAsync(300));
    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith({ name: "abc" });
    expect(result.current.status).toBe("saved");
  });

  it("reports errors from the save function", async () => {
    const save = vi.fn().mockRejectedValue(new Error("disk full"));
    const { result, rerender } = renderHook(({ v }) => useAutosave(v, save, 100), {
      initialProps: { v: "a" },
    });
    rerender({ v: "b" });
    await act(() => vi.advanceTimersByTimeAsync(150));
    expect(result.current.status).toBe("error");
    expect(result.current.error).toBe("disk full");
  });

  it("flushes a pending change on unmount", () => {
    const save = vi.fn().mockResolvedValue(undefined);
    const { rerender, unmount } = renderHook(({ v }) => useAutosave(v, save, 500), {
      initialProps: { v: "a" },
    });
    rerender({ v: "b" });
    unmount();
    expect(save).toHaveBeenCalledWith("b");
  });
});
