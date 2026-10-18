import { open } from "@tauri-apps/plugin-dialog";
import { e2eDir } from "@/lib/e2e";

/**
 * Folder picker for the vault to import. Resolves to `null` if cancelled.
 * Debug builds started with ORBIS_E2E_DIR answer `<dir>/vault` instead of
 * showing the native dialog (the test creates that folder first).
 */
export async function pickVaultFolder(): Promise<string | null> {
  const hooked = await e2eDir();
  if (hooked) return `${hooked}/vault`;
  const result = await open({
    directory: true,
    multiple: false,
    title: "Choose the Markdown or Obsidian vault to import",
  });
  return typeof result === "string" ? result : null;
}
