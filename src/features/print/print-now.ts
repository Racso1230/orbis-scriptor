import { e2eDir } from "@/lib/e2e";

/**
 * Opens the system print dialog ("Save as PDF" is one of its printers).
 * Waits for web fonts first. Skipped in the end-to-end tests, which cannot
 * click a native dialog (debug builds started with ORBIS_E2E_DIR).
 */
export async function printNow(): Promise<void> {
  if (await e2eDir()) return;
  try {
    await document.fonts?.ready;
  } catch {
    // fonts API unavailable: print anyway
  }
  window.print();
}
