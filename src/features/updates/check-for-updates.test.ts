import { beforeEach, describe, expect, it, vi } from "vitest";
import { runUpdateCheck } from "./check-for-updates";

const mocks = vi.hoisted(() => ({
  check: vi.fn(),
  relaunch: vi.fn(),
  toast: Object.assign(vi.fn(), {
    success: vi.fn(),
    error: vi.fn(),
    loading: vi.fn(),
    dismiss: vi.fn(),
  }),
}));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: mocks.check }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: mocks.relaunch }));
vi.mock("sonner", () => ({ toast: mocks.toast }));

describe("runUpdateCheck", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(console, "warn").mockImplementation(() => {});
  });

  it("stays silent on startup when there is no update", async () => {
    mocks.check.mockResolvedValue(null);
    expect(await runUpdateCheck({ manual: false })).toBe(false);
    expect(mocks.toast.success).not.toHaveBeenCalled();
  });

  it("says 'up to date' on a manual check", async () => {
    mocks.check.mockResolvedValue(null);
    await runUpdateCheck({ manual: true });
    expect(mocks.toast.success).toHaveBeenCalledOnce();
  });

  it("offers install and restart when an update exists", async () => {
    const update = { version: "0.2.0", downloadAndInstall: vi.fn().mockResolvedValue(undefined) };
    mocks.check.mockResolvedValue(update);
    expect(await runUpdateCheck({ manual: false })).toBe(true);
    const options = mocks.toast.mock.calls[0]?.[1] as { action: { onClick: () => void } };
    options.action.onClick();
    await vi.waitFor(() => expect(mocks.relaunch).toHaveBeenCalled());
    expect(update.downloadAndInstall).toHaveBeenCalled();
  });

  it("is quiet about failures at startup (offline is fine)", async () => {
    mocks.check.mockRejectedValue(new Error("network"));
    expect(await runUpdateCheck({ manual: false })).toBe(false);
    expect(mocks.toast.error).not.toHaveBeenCalled();
  });

  it("reports failures on a manual check", async () => {
    mocks.check.mockRejectedValue(new Error("network"));
    await runUpdateCheck({ manual: true });
    expect(mocks.toast.error).toHaveBeenCalledOnce();
  });
});
