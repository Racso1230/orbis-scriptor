import { act, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import { useSettings, useUpdateSettings } from "./queries";

const getSettings = vi.fn();
const updateSettings = vi.fn();
vi.mock("@/lib/bindings", () => ({
  commands: {
    getSettings: () => getSettings(),
    updateSettings: (s: unknown) => updateSettings(s),
  },
}));

function wrapper() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

describe("settings queries", () => {
  it("fills in defaults for fields Rust left out", async () => {
    getSettings.mockResolvedValue({ status: "ok", data: { theme: "dark" } });
    const { result } = renderHook(() => useSettings(), { wrapper: wrapper() });
    await waitFor(() => expect(result.current.data).toBeDefined());
    expect(result.current.data).toEqual({
      theme: "dark",
      autosaveDelayMs: 800,
      editorFontSize: 16,
    });
  });

  it("updates the cache with the clamped values Rust returns", async () => {
    getSettings.mockResolvedValue({ status: "ok", data: {} });
    const clamped = { theme: "light", autosaveDelayMs: 300, editorFontSize: 24 };
    updateSettings.mockResolvedValue({ status: "ok", data: clamped });
    const { result } = renderHook(() => ({ q: useSettings(), m: useUpdateSettings() }), {
      wrapper: wrapper(),
    });
    await waitFor(() => expect(result.current.q.data).toBeDefined());
    await act(() =>
      result.current.m.mutateAsync({ theme: "light", autosaveDelayMs: 5, editorFontSize: 99 }),
    );
    await waitFor(() => expect(result.current.q.data).toEqual(clamped));
  });
});
