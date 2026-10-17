import { spawn, type ChildProcess } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { chromium, type Browser, type Page } from "@playwright/test";

const ROOT = path.resolve(import.meta.dirname, "..");
const EXE = path.join(ROOT, "src-tauri", "target", "debug", "orbis-scriptor.exe");
const SAMPLE_MAP = path.join(ROOT, "src-tauri", "assets", "shattered-coast-map.png");
const PORT = Number(process.env.E2E_CDP_PORT ?? 9222);

export interface RunningApp {
  page: Page;
  /** Temp directory handed to the app through ORBIS_E2E_DIR. */
  dir: string;
  stop: () => Promise<void>;
}

/** Polls the DevTools endpoint until WebView2 is listening. */
async function waitForCdp(app: ChildProcess): Promise<void> {
  const deadline = Date.now() + 90_000;
  while (Date.now() < deadline) {
    if (app.exitCode !== null) throw new Error(`App exited early (code ${app.exitCode})`);
    try {
      const res = await fetch(`http://127.0.0.1:${PORT}/json/version`);
      if (res.ok) return;
    } catch {
      // not listening yet
    }
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error(`WebView2 DevTools port ${PORT} did not open`);
}

/**
 * Starts the debug build of Orbis Scriptor with WebView2 remote debugging on,
 * and connects Playwright to its window.
 *
 * ORBIS_E2E_DIR makes the (debug-only) pick helpers skip the native dialogs:
 * the folder picker answers `<dir>/world` and the image picker `<dir>/map.png`.
 */
export async function startApp(): Promise<RunningApp> {
  if (process.platform !== "win32") {
    throw new Error("The e2e tests drive WebView2 and only run on Windows.");
  }
  if (!existsSync(EXE)) {
    throw new Error(`Debug build not found at ${EXE}. Run "npm run e2e" to build it first.`);
  }

  const dir = mkdtempSync(path.join(tmpdir(), "orbis-e2e-"));
  copyFileSync(SAMPLE_MAP, path.join(dir, "map.png"));
  const webviewData = path.join(dir, "webview-data");
  mkdirSync(webviewData);

  const child = spawn(EXE, [], {
    stdio: "ignore",
    // Debug builds regenerate ../src/lib/bindings.ts relative to the working
    // directory on start-up, so run from src-tauri exactly like `tauri dev` does.
    cwd: path.join(ROOT, "src-tauri"),
    env: {
      ...process.env,
      ORBIS_E2E_DIR: dir,
      // A clean WebView2 profile, so no localStorage (tabs, theme) leaks between runs.
      WEBVIEW2_USER_DATA_FOLDER: webviewData,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${PORT}`,
    },
  });

  let browser: Browser | undefined;
  try {
    await waitForCdp(child);
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
    const context = browser.contexts()[0];
    if (!context) throw new Error("No WebView2 context found");
    const page = context.pages()[0] ?? (await context.waitForEvent("page"));
    await page.waitForLoadState("domcontentloaded");

    return {
      page,
      dir,
      stop: async () => {
        await browser?.close().catch(() => {});
        // Kill the whole process tree: WebView2 spawns helper processes.
        if (child.pid) {
          await new Promise<void>((resolve) =>
            spawn("taskkill", ["/pid", String(child.pid), "/T", "/F"], { stdio: "ignore" }).on(
              "exit",
              () => resolve(),
            ),
          );
        }
        await new Promise((r) => setTimeout(r, 1000));
        rmSync(dir, { recursive: true, force: true, maxRetries: 5, retryDelay: 500 });
      },
    };
  } catch (error) {
    await browser?.close().catch(() => {});
    child.kill();
    throw error;
  }
}
