import { beforeEach, describe, expect, it, vi } from "vitest";

const { e2eDir, save } = vi.hoisted(() => ({ e2eDir: vi.fn(), save: vi.fn() }));
vi.mock("@/lib/e2e", () => ({ e2eDir }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save }));

import { pickExportPath } from "./export-path";

describe("pickExportPath", () => {
  beforeEach(() => {
    e2eDir.mockReset();
    save.mockReset();
  });

  it("answers <e2eDir>/export.png without a dialog when the test hook is on", async () => {
    e2eDir.mockResolvedValue("C:/tmp/orbis-e2e-1");
    await expect(pickExportPath("Map.png")).resolves.toBe("C:/tmp/orbis-e2e-1/export.png");
    expect(save).not.toHaveBeenCalled();
  });

  it("shows the native save dialog with a png filter and default name otherwise", async () => {
    e2eDir.mockResolvedValue(null);
    save.mockResolvedValue("D:/maps/Realm.png");
    await expect(pickExportPath("Realm.png")).resolves.toBe("D:/maps/Realm.png");
    expect(save).toHaveBeenCalledWith(
      expect.objectContaining({
        defaultPath: "Realm.png",
        filters: [{ name: "PNG image", extensions: ["png"] }],
      }),
    );
  });

  it("returns null when the dialog is cancelled", async () => {
    e2eDir.mockResolvedValue(null);
    save.mockResolvedValue(null);
    await expect(pickExportPath("Realm.png")).resolves.toBeNull();
  });
});
