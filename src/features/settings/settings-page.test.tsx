import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithQuery } from "@/test/render";
import { useUiStore } from "@/stores/ui-store";
import { SettingsPage } from "./settings-page";

const commands = vi.hoisted(() => ({ getSettings: vi.fn(), updateSettings: vi.fn() }));
vi.mock("@/lib/bindings", () => ({ commands }));
vi.mock("@tanstack/react-router", () => ({
  useRouter: () => ({ history: { back: vi.fn() } }),
}));

const SAVED = { theme: "light", autosaveDelayMs: 2000, editorFontSize: 18 };

describe("SettingsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    };
    useUiStore.setState({ theme: "light" });
    commands.getSettings.mockResolvedValue({ status: "ok", data: SAVED });
    commands.updateSettings.mockImplementation((s: unknown) =>
      Promise.resolve({ status: "ok", data: s }),
    );
  });

  it("shows the saved values", async () => {
    renderWithQuery(<SettingsPage />);
    expect(await screen.findByText("2.0 s")).toBeInTheDocument();
    expect(screen.getByText("18 px")).toBeInTheDocument();
    expect(screen.getByTestId("font-preview")).toHaveStyle({ fontSize: "18px" });
    expect(screen.getByRole("radio", { name: /Light/ })).toBeChecked();
  });

  it("saves the full settings object when the font size changes", async () => {
    renderWithQuery(<SettingsPage />);
    await screen.findByText("18 px");
    const slider = screen.getByRole("slider", { name: "Editor font size" });
    fireEvent.keyDown(slider, { key: "ArrowRight" });
    await screen.findByText("19 px");
    await waitFor(() =>
      expect(commands.updateSettings).toHaveBeenCalledWith({ ...SAVED, editorFontSize: 19 }),
    );
  });

  it("saves the full settings object when the theme changes", async () => {
    renderWithQuery(<SettingsPage />);
    await screen.findByText("18 px");
    fireEvent.click(screen.getByRole("radio", { name: /Dark/ }));
    expect(useUiStore.getState().theme).toBe("dark");
    await waitFor(() =>
      expect(commands.updateSettings).toHaveBeenCalledWith({ ...SAVED, theme: "dark" }),
    );
  });

  it("lists the keyboard shortcuts", async () => {
    renderWithQuery(<SettingsPage />);
    expect(await screen.findByText("Save now")).toBeInTheDocument();
    expect(screen.getByText("Close the current tab")).toBeInTheDocument();
  });
});
