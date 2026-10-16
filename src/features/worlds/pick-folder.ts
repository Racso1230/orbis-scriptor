import { open } from "@tauri-apps/plugin-dialog";

/** Opens the native folder picker. Resolves to `null` if the user cancels. */
export async function pickFolder(title: string): Promise<string | null> {
  const result = await open({ directory: true, multiple: false, title });
  return typeof result === "string" ? result : null;
}

/** Last segment of a path, used as the default name for a new world. */
export function folderName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? "";
}
