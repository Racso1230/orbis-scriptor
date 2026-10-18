import { open } from "@tauri-apps/plugin-dialog";
import { e2eDir } from "@/lib/e2e";

/**
 * Native file picker for gallery images (several at once). Resolves to an
 * empty list if cancelled. Debug builds started with ORBIS_E2E_DIR answer
 * `<dir>/map.png` instead of showing the dialog.
 */
export async function pickImageFiles(): Promise<string[]> {
  const hooked = await e2eDir();
  if (hooked) return [`${hooked}/map.png`];
  const result = await open({
    multiple: true,
    directory: false,
    title: "Add images",
    filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp"] }],
  });
  if (Array.isArray(result)) return result;
  return typeof result === "string" ? [result] : [];
}
