import { copyFileSync, readFileSync } from "node:fs";
import path from "node:path";
import { expect, test, type Page } from "@playwright/test";
import { startApp, type RunningApp } from "./app";

/**
 * The sample world's image map: labels curved along the river, stamps, and a
 * PNG export of a map with a background IMAGE (checks the exported canvas is
 * not tainted by the asset protocol). Set E2E_SHOTS to save screenshots.
 */

let app: RunningApp;
let page: Page;

test.beforeAll(async () => {
  app = await startApp();
  page = app.page;
});

test.afterAll(async () => {
  await app?.stop();
});

test("sample map renders labels and stamps, and exports a PNG", async () => {
  await page.getByRole("button", { name: "Create sample world" }).click();
  // The sidebar list is virtualised and now starts with Favourites and Recently
  // edited, so the map row may be out of view: open it from the palette.
  await expect(page.getByRole("button", { name: "Timeline" })).toBeVisible();
  await page.keyboard.press("Control+k");
  await page.getByPlaceholder(/Search entities/).fill("Map of the Shattered Coast");
  await page
    .getByRole("option", { name: /Map of the Shattered Coast/ })
    .first()
    .click();
  await expect(page.getByRole("button", { name: /^Path/ })).toBeVisible();
  // The image has loaded once the "Loading map image" notice is gone.
  await expect(page.getByText("Loading map image")).toHaveCount(0);
  await page.waitForTimeout(800);

  const shots = process.env.E2E_SHOTS;
  if (shots) await page.screenshot({ path: path.join(shots, "sample-map.png") });

  await page.getByRole("button", { name: "Export PNG…" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("button", { name: "Export…" }).click();
  await expect(page.getByText("Map exported")).toBeVisible({ timeout: 60_000 });

  const file = path.join(app.dir, "export.png");
  const png = readFileSync(file);
  expect([...png.subarray(0, 8)]).toEqual([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  expect(png.length).toBeGreaterThan(50_000);
  if (shots) copyFileSync(file, path.join(shots, "sample-export.png"));
});
