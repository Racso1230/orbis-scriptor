import { save } from "@tauri-apps/plugin-dialog";
import { e2eDir } from "@/lib/e2e";

/**
 * Asks where to save the exported PNG. Resolves to `null` if the user cancels.
 *
 * Test hook: debug builds started with ORBIS_E2E_DIR skip the native dialog and
 * answer `<dir>/export.png` (release builds always show the dialog).
 */
export async function pickExportPath(defaultName: string): Promise<string | null> {
  const hooked = await e2eDir();
  if (hooked) return `${hooked}/export.png`;
  const result = await save({
    title: "Export map as PNG",
    defaultPath: defaultName,
    filters: [{ name: "PNG image", extensions: ["png"] }],
  });
  return typeof result === "string" ? result : null;
}
