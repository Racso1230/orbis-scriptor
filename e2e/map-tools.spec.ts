import { readFileSync } from "node:fs";
import path from "node:path";
import { expect, test, type Locator, type Page } from "@playwright/test";
import { startApp, type RunningApp } from "./app";

/**
 * Map editor tools on a blank parchment map: freehand, dragging objects,
 * vertex handles, labels (straight and along a path), stamps, undo and PNG
 * export. Set E2E_SHOTS to a folder to also save screenshots there.
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

async function shot(name: string) {
  const dir = process.env.E2E_SHOTS;
  if (dir) await page.screenshot({ path: path.join(dir, `${name}.png`) });
}

interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Screen position at a fraction of the canvas. */
function at(box: Box, fx: number, fy: number) {
  return { x: box.x + box.width * fx, y: box.y + box.height * fy };
}

/** Parses "Position: 123, 456". */
async function position(properties: Locator): Promise<[number, number]> {
  const text = (await properties.getByText(/^Position:/).textContent()) ?? "";
  const match = /(-?\d+), (-?\d+)/.exec(text);
  if (!match) throw new Error(`No position in "${text}"`);
  return [Number(match[1]), Number(match[2])];
}

/** Parses "N points, M px long". */
async function pathInfo(properties: Locator): Promise<{ points: number; length: number }> {
  const text = (await properties.getByText(/points, \d+ px long/).textContent()) ?? "";
  const match = /(\d+) points, (\d+) px long/.exec(text);
  if (!match) throw new Error(`No path info in "${text}"`);
  return { points: Number(match[1]), length: Number(match[2]) };
}

test("draw, drag, label, stamp, undo and export on a blank map", async () => {
  // A world with a blank map.
  await page.getByRole("button", { name: "New world" }).click();
  await page.getByLabel("World name").fill("Tools World");
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Your world is empty" })).toBeVisible();
  await page.getByRole("button", { name: "New map" }).first().click();
  const nameInput = page.getByRole("textbox", { name: "Name" });
  await expect(nameInput).toHaveValue(/^New map$/i);
  await nameInput.fill("Tools Map");
  await page.getByRole("button", { name: "Start blank" }).click();
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await expect(page.getByRole("button", { name: "Map settings" })).toBeVisible();

  // Fit the 2048 x 1536 map (the default size) and work out where it is on screen,
  // so the fractions below are fractions of the MAP, not of the whole canvas.
  await page.getByRole("button", { name: "Fit to screen" }).click();
  const rawBox = await page.locator("canvas").first().boundingBox();
  if (!rawBox) throw new Error("map canvas has no bounding box");
  const fit = Math.min((rawBox.width - 48) / 2048, (rawBox.height - 48) / 1536);
  const box: Box = {
    x: rawBox.x + (rawBox.width - 2048 * fit) / 2,
    y: rawBox.y + (rawBox.height - 1536 * fit) / 2,
    width: 2048 * fit,
    height: 1536 * fit,
  };
  const properties = page.getByRole("region", { name: "Properties" });
  const undo = page.getByRole("button", { name: "Undo" });

  // 1. Freehand: a wavy stroke of many samples is saved as a simplified path.
  await page.getByRole("button", { name: /^Freehand/ }).click();
  const start = at(box, 0.15, 0.7);
  await page.mouse.move(start.x, start.y);
  await page.mouse.down();
  const samples = 60;
  for (let i = 1; i <= samples; i++) {
    const t = i / samples;
    await page.mouse.move(start.x + t * box.width * 0.5, start.y + Math.sin(t * Math.PI * 2) * 25);
  }
  await page.mouse.up();
  await expect(properties.getByRole("heading", { level: 3 })).toHaveText("Path properties");
  const stroke = await pathInfo(properties);
  expect(stroke.points).toBeGreaterThanOrEqual(2);
  expect(stroke.points).toBeLessThan(samples / 2);
  await shot("tools-1-freehand");

  // 2. Path tool: a three-point river to drag a vertex of later.
  await page.getByRole("button", { name: /^Path/ }).click();
  const a = at(box, 0.2, 0.2);
  const b = at(box, 0.4, 0.4);
  const c = at(box, 0.6, 0.2);
  await page.mouse.click(a.x, a.y);
  await page.mouse.click(b.x, b.y);
  await page.mouse.dblclick(c.x, c.y);
  await expect(properties.getByRole("heading", { level: 3 })).toHaveText("Path properties");
  expect((await pathInfo(properties)).points).toBe(3);

  // 3. Label tool: a label on empty ground, then one that follows the river.
  await page.getByRole("button", { name: /^Label/ }).click();
  const free = at(box, 0.75, 0.75);
  await page.mouse.click(free.x, free.y);
  const labelBox = page.getByRole("textbox", { name: "Label text" });
  await expect(labelBox).toBeFocused();
  await page.keyboard.type("Eastmarch");
  await page.keyboard.press("Enter");
  await expect(properties.getByRole("heading", { level: 3 })).toHaveText("Label properties");
  await expect(properties.getByLabel("Text", { exact: true })).toHaveValue("Eastmarch");
  await expect(properties.getByRole("button", { name: "Detach from path" })).toHaveCount(0);

  // Click on the river (the first vertex is on the curve) to attach the label.
  await page.mouse.click(a.x, a.y);
  await page.keyboard.type("River Test");
  await page.keyboard.press("Enter");
  await expect(properties.getByLabel("Text", { exact: true })).toHaveValue("River Test");
  await expect(properties.getByRole("button", { name: "Detach from path" })).toBeVisible();
  await shot("tools-2-labels");

  // Esc cancels an empty text box without creating anything.
  await page.mouse.click(at(box, 0.9, 0.9).x, at(box, 0.9, 0.9).y);
  await expect(labelBox).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(labelBox).toHaveCount(0);

  // 4. Select tool: double-click the straight label to edit its text inline.
  await page.getByRole("button", { name: /^Select/ }).click();
  await page.mouse.dblclick(free.x, free.y);
  await expect(labelBox).toBeFocused();
  await expect(labelBox).toHaveValue("Eastmarch");
  await labelBox.fill("Westmarch");
  await page.keyboard.press("Enter");
  await expect(properties.getByLabel("Text", { exact: true })).toHaveValue("Westmarch");

  // 5. Stamps: pick Castle in the palette and place two.
  await page.getByRole("button", { name: /^Stamp/ }).click();
  const palette = page.getByRole("group", { name: "Stamps" });
  await expect(palette.getByRole("button", { name: "Mountain", exact: true })).toBeVisible();
  await palette.getByRole("button", { name: "Castle" }).click();
  const castle = at(box, 0.3, 0.55);
  await page.mouse.click(castle.x, castle.y);
  await expect(properties.getByRole("heading", { level: 3 })).toHaveText("Stamp properties");
  await expect(properties.getByLabel("Icon")).toHaveValue("castle");
  await palette.getByRole("button", { name: "Forest", exact: true }).click();
  const forest = at(box, 0.85, 0.35);
  await page.mouse.click(forest.x, forest.y);
  await expect(properties.getByLabel("Icon")).toHaveValue("forest");
  await shot("tools-3-stamps");

  // Esc leaves the Stamp tool.
  await page.keyboard.press("Escape");
  await expect(page.getByRole("group", { name: "Stamps" })).toHaveCount(0);

  // 6. Drag the castle: its position changes; undo puts it back.
  await page.mouse.click(castle.x, castle.y);
  await expect(properties.getByRole("heading", { level: 3 })).toHaveText("Stamp properties");
  const before = await position(properties);
  await page.mouse.move(castle.x, castle.y);
  await page.mouse.down();
  await page.mouse.move(castle.x + 40, castle.y + 20, { steps: 8 });
  await page.mouse.move(castle.x + 80, castle.y + 40, { steps: 8 });
  await page.mouse.up();
  await expect(async () => {
    const after = await position(properties);
    expect(after[0]).toBeGreaterThan(before[0] + 20);
    expect(after[1]).toBeGreaterThan(before[1] + 10);
  }).toPass();
  await undo.click();
  await expect(async () => {
    expect(await position(properties)).toEqual(before);
  }).toPass();

  // 7. Vertex handle: select the river and drag its first vertex (the label sits on the middle).
  await page.mouse.click(a.x, a.y);
  await expect(properties.getByRole("heading", { level: 3 })).toHaveText("Path properties");
  const lengthBefore = (await pathInfo(properties)).length;
  await page.mouse.move(a.x, a.y);
  await page.mouse.down();
  await page.mouse.move(a.x + 10, a.y + 40, { steps: 8 });
  await page.mouse.move(a.x + 20, a.y + 90, { steps: 8 });
  await page.mouse.up();
  await expect(async () => {
    expect(Math.abs((await pathInfo(properties)).length - lengthBefore)).toBeGreaterThan(20);
  }).toPass();
  await shot("tools-4-vertex");
  await undo.click();
  await expect(async () => {
    expect((await pathInfo(properties)).length).toBe(lengthBefore);
  }).toPass();

  // 8. Export PNG: the native save dialog is answered by ORBIS_E2E_DIR.
  await page.getByRole("button", { name: "Export PNG…" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByText(/Image size: \d+ × \d+ px/)).toBeVisible();
  const sizeText = (await dialog.getByText(/Image size:/).textContent()) ?? "";
  const size = /(\d+) × (\d+) px/.exec(sizeText);
  if (!size) throw new Error(`No size in "${sizeText}"`);
  await dialog.getByRole("button", { name: "Export…" }).click();
  await expect(page.getByText("Map exported")).toBeVisible({ timeout: 60_000 });

  const png = readFileSync(path.join(app.dir, "export.png"));
  expect([...png.subarray(0, 8)]).toEqual([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  expect(png.length).toBeGreaterThan(10_000);
  // IHDR: width and height are the big-endian u32s at bytes 16 and 20.
  expect(png.readUInt32BE(16)).toBe(Number(size[1]));
  expect(png.readUInt32BE(20)).toBe(Number(size[2]));
  const shots = process.env.E2E_SHOTS;
  if (shots) {
    const { copyFileSync } = await import("node:fs");
    copyFileSync(path.join(app.dir, "export.png"), path.join(shots, "tools-export.png"));
  }
});
