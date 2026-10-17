import { defineConfig } from "@playwright/test";

// The end-to-end test launches the real desktop app itself (see e2e/app.ts) and
// attaches over the WebView2 DevTools port, so there is no `webServer` and no
// browser project here.
export default defineConfig({
  testDir: "./e2e",
  testMatch: "**/*.spec.ts",
  // One app instance, one test at a time.
  workers: 1,
  fullyParallel: false,
  timeout: 120_000,
  expect: { timeout: 10_000 },
  retries: 0,
  // On CI the github reporter also turns failures into run annotations.
  reporter: process.env.CI ? [["list"], ["github"]] : [["list"]],
});
