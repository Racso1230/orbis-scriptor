import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
} from "node:fs";
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

/** Lists files under `dir` (relative paths), for failure diagnostics. */
function listFiles(dir: string, depth = 2): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory() && depth > 0) {
      return [entry.name + "/", ...listFiles(full, depth - 1).map((p) => `${entry.name}/${p}`)];
    }
    return [entry.name];
  });
}

/**
 * WebView2 writes the port it actually opened to `DevToolsActivePort` in its
 * profile folder. Reading it avoids guessing if the requested port was taken.
 */
function activePort(webviewData: string): number | undefined {
  const file = path.join(webviewData, "EBWebView", "DevToolsActivePort");
  if (!existsSync(file)) return undefined;
  const port = Number(readFileSync(file, "utf8").split(/\r?\n/)[0]);
  return Number.isFinite(port) && port > 0 ? port : undefined;
}

/** Polls until WebView2's DevTools endpoint answers; returns its port. */
async function waitForCdp(app: ChildProcess, webviewData: string, output: string[]) {
  const deadline = Date.now() + 90_000;
  while (Date.now() < deadline) {
    if (app.exitCode !== null) {
      throw new Error(`App exited early (code ${app.exitCode}).\n${tail(output)}`);
    }
    for (const port of new Set([activePort(webviewData), PORT])) {
      if (port === undefined) continue;
      try {
        const res = await fetch(`http://127.0.0.1:${port}/json/version`);
        if (res.ok) return port;
      } catch {
        // not listening yet
      }
    }
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error(
    [
      `WebView2 DevTools port ${PORT} did not open.`,
      webviewProcesses(),
      edgePolicies(),
      tail(output),
      `WebView2 profile (top level): ${listFiles(path.join(webviewData, "EBWebView"), 0).join(", ") || "(none — WebView2 never started)"}`,
    ].join("\n"),
  );
}

/** Runs a PowerShell snippet for diagnostics; never throws. */
function powershell(script: string): string {
  try {
    return execFileSync("powershell", ["-NoProfile", "-Command", script], {
      encoding: "utf8",
      timeout: 20_000,
    }).trim();
  } catch (error) {
    return `(failed: ${String(error)})`;
  }
}

/** Command lines of the WebView2 browser processes: did our flag reach them? */
function webviewProcesses(): string {
  const out = powershell(
    "Get-CimInstance Win32_Process -Filter \"name='msedgewebview2.exe'\" | " +
      "Where-Object { $_.CommandLine -notmatch '--type=' } | " +
      "ForEach-Object { $_.CommandLine }",
  );
  return `WebView2 browser process command lines:\n${out || "(none running)"}`;
}

/** Edge / WebView2 group policies, which can disable remote debugging. */
function edgePolicies(): string {
  const out = powershell(
    "foreach ($k in 'HKLM:\\SOFTWARE\\Policies\\Microsoft\\Edge','HKLM:\\SOFTWARE\\Policies\\Microsoft\\Edge\\WebView2'," +
      "'HKCU:\\SOFTWARE\\Policies\\Microsoft\\Edge','HKCU:\\SOFTWARE\\Policies\\Microsoft\\Edge\\WebView2') " +
      '{ if (Test-Path $k) { "[$k]"; (Get-ItemProperty $k | Out-String).Trim() } }',
  );
  return `Edge/WebView2 policies:\n${out || "(none)"}`;
}

/** The last lines the app printed, for error messages. */
function tail(output: string[]): string {
  const lines = output.join("").split(/\r?\n/).filter(Boolean).slice(-40);
  return lines.length
    ? `App output (last ${lines.length} lines):\n${lines.join("\n")}`
    : "App printed nothing.";
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

  const output: string[] = [];
  const child = spawn(EXE, [], {
    // Keep the app's own logs so a failed start can say why.
    stdio: ["ignore", "pipe", "pipe"],
    // Debug builds regenerate ../src/lib/bindings.ts relative to the working
    // directory on start-up, so run from src-tauri exactly like `tauri dev` does.
    cwd: path.join(ROOT, "src-tauri"),
    env: {
      ...process.env,
      ORBIS_E2E_DIR: dir,
      // A clean WebView2 profile, so no localStorage (tabs, theme) leaks between runs.
      WEBVIEW2_USER_DATA_FOLDER: webviewData,
      // The debug app reads this and enables WebView2's DevTools port itself
      // (see app_context in src-tauri/src/lib.rs); the env var below is kept
      // as a fallback for WebView2 versions that honour it.
      E2E_CDP_PORT: String(PORT),
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${PORT}`,
    },
  });

  child.stdout?.on("data", (chunk: Buffer) => output.push(chunk.toString()));
  child.stderr?.on("data", (chunk: Buffer) => output.push(chunk.toString()));

  let browser: Browser | undefined;
  try {
    const port = await waitForCdp(child, webviewData, output);
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
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
