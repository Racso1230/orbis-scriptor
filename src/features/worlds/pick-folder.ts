import { e2eDir } from "@/lib/e2e";
import { open } from "@tauri-apps/plugin-dialog";

/** Opens the native folder picker. Resolves to `null` if the user cancels. */
export async function pickFolder(title: string): Promise<string | null> {
  // Test hook: skip the native dialog (debug builds with ORBIS_E2E_DIR only).
  const hooked = await e2eDir();
  if (hooked) return `${hooked}/world`;
  const result = await open({ directory: true, multiple: false, title });
  return typeof result === "string" ? result : null;
}

/** Last segment of a path, used as the default name for a new world. */
export function folderName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? "";
}
