import { commands } from "@/lib/bindings";

/**
 * Test hook for the Playwright suite (see README, "End-to-end tests").
 *
 * Native dialogs cannot be automated, so when the debug build is started with
 * `ORBIS_E2E_DIR` set, the pick helpers use paths inside that folder instead.
 * The Rust side returns `null` in release builds, so this is inert for users.
 *
 * Any failure (for example, a unit test with partly mocked bindings) means
 * "no hook", and the real dialog is shown.
 */
export async function e2eDir(): Promise<string | null> {
  try {
    return (await commands.e2eDir()) ?? null;
  } catch {
    return null;
  }
}
