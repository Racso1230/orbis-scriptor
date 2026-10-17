import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { toast } from "sonner";
import { toastError } from "@/lib/notify";

const UPDATE_TOAST_ID = "app-update";

/** Downloads and installs `update`, then restarts the app into the new version. */
export async function installUpdate(update: Update): Promise<void> {
  toast.loading("Downloading update…", { id: UPDATE_TOAST_ID, duration: Infinity });
  try {
    await update.downloadAndInstall();
    await relaunch();
  } catch (error) {
    toast.dismiss(UPDATE_TOAST_ID);
    toastError(error, UPDATE_TOAST_ID);
  }
}

/**
 * Asks GitHub Releases whether a newer version exists.
 *
 * - `manual: true` (the user clicked "Check for updates"): always tells them the
 *   outcome, including "up to date" and failures.
 * - `manual: false` (startup check): says nothing unless an update exists. The
 *   app is local-first, so being offline is normal and only gets a console note.
 *
 * Resolves to `true` when an update is available.
 */
export async function runUpdateCheck({ manual }: { manual: boolean }): Promise<boolean> {
  try {
    const update = await check();
    if (!update) {
      if (manual) toast.success("You're up to date", { description: "No newer version found." });
      return false;
    }
    toast(`Version ${update.version} is available`, {
      id: UPDATE_TOAST_ID,
      description: "Your worlds are not touched by updates.",
      duration: Infinity,
      action: { label: "Install and restart", onClick: () => void installUpdate(update) },
    });
    return true;
  } catch (error) {
    if (manual) {
      toast.error("Could not check for updates", {
        description: "Check your internet connection and try again.",
      });
    }
    console.warn("Update check failed:", error);
    return false;
  }
}
