import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithQuery } from "@/test/render";
import { WelcomePage } from "./welcome-page";

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const commands = vi.hoisted(() => ({
  appInfo: vi.fn(),
  currentWorld: vi.fn(),
  listRecentWorlds: vi.fn(),
  openWorld: vi.fn(),
  createWorld: vi.fn(),
  createSampleWorld: vi.fn(),
  removeRecentWorld: vi.fn(),
}));

vi.mock("@/lib/bindings", () => ({ commands }));
const dialogOpen = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: dialogOpen }));
// WelcomePage calls useNavigate; outside a RouterProvider we stub it.
vi.mock("@tanstack/react-router", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@tanstack/react-router")>()),
  useNavigate: () => vi.fn().mockResolvedValue(undefined),
}));

describe("WelcomePage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    dialogOpen.mockResolvedValue(null);
    commands.appInfo.mockResolvedValue(ok({ name: "Orbis Scriptor", version: "0.1.0" }));
    commands.currentWorld.mockResolvedValue(ok(null));
    commands.listRecentWorlds.mockResolvedValue(
      ok([
        { name: "Aerth", path: "C:/worlds/aerth", lastOpenedAt: "", available: true },
        { name: "Lost", path: "C:/worlds/lost", lastOpenedAt: "", available: false },
      ]),
    );
    commands.openWorld.mockResolvedValue(ok({ name: "Aerth", path: "C:/worlds/aerth" }));
    commands.removeRecentWorld.mockResolvedValue(ok(null));
  });

  it("lists recent worlds and disables unavailable ones", async () => {
    renderWithQuery(<WelcomePage />);

    expect(await screen.findByText("Aerth")).toBeInTheDocument();
    expect(screen.getByText("(unavailable)")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Lost/ })).toBeDisabled();
    expect(screen.getByRole("button", { name: /^Aerth/ })).toBeEnabled();
  });

  it("opens a recent world when clicked", async () => {
    renderWithQuery(<WelcomePage />);

    fireEvent.click(await screen.findByRole("button", { name: /^Aerth/ }));

    await waitFor(() => expect(commands.openWorld).toHaveBeenCalledWith("C:/worlds/aerth"));
  });

  it("removes a recent world", async () => {
    renderWithQuery(<WelcomePage />);

    fireEvent.click(await screen.findByRole("button", { name: "Remove Lost from recent worlds" }));

    await waitFor(() => expect(commands.removeRecentWorld).toHaveBeenCalledWith("C:/worlds/lost"));
  });

  it("creates the sample world inside the chosen folder", async () => {
    dialogOpen.mockResolvedValue("C:/worlds");
    commands.createSampleWorld.mockResolvedValue(
      ok({ name: "The Shattered Coast", path: "C:/worlds/The Shattered Coast" }),
    );
    renderWithQuery(<WelcomePage />);

    fireEvent.click(await screen.findByRole("button", { name: /Create sample world/ }));

    await waitFor(() => expect(commands.createSampleWorld).toHaveBeenCalledWith("C:/worlds"));
  });

  it("does nothing when the sample world folder picker is cancelled", async () => {
    renderWithQuery(<WelcomePage />);

    fireEvent.click(await screen.findByRole("button", { name: /Create sample world/ }));

    await waitFor(() => expect(dialogOpen).toHaveBeenCalled());
    expect(commands.createSampleWorld).not.toHaveBeenCalled();
  });

  it("shows the error message when opening fails", async () => {
    commands.openWorld.mockResolvedValue({
      status: "error" as const,
      error: { kind: "notFound" as const, message: "No world.json found" },
    });
    renderWithQuery(<WelcomePage />);

    fireEvent.click(await screen.findByRole("button", { name: /^Aerth/ }));

    expect(await screen.findByRole("alert")).toHaveTextContent("No world.json found");
  });
});
