import { open } from "@tauri-apps/plugin-dialog";
import { e2eDir } from "@/lib/e2e";

/**
 * Folder picker for the Markdown export. Resolves to `null` if cancelled.
 * Debug builds started with ORBIS_E2E_DIR answer `<dir>/export-md` instead of
 * showing the native dialog (the test creates that folder first).
 */
export async function pickExportFolder(): Promise<string | null> {
  const hooked = await e2eDir();
  if (hooked) return `${hooked}/export-md`;
  const result = await open({
    directory: true,
    multiple: false,
    title: "Choose where to export the world",
  });
  return typeof result === "string" ? result : null;
}
