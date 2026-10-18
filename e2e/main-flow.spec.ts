import { expect, test, type Page } from "@playwright/test";
import { startApp, type RunningApp } from "./app";

let app: RunningApp;
let page: Page;

test.beforeAll(async () => {
  app = await startApp();
  page = app.page;
});

test.afterAll(async () => {
  await app?.stop();
});

/** Waits for the entity page's autosave indicator to settle. */
async function expectSaved() {
  await expect(page.getByRole("status").filter({ hasText: /^Saved$/ })).toBeVisible();
}

/** Creates an entity from the sidebar "+" button and renames it. */
async function createEntity(typeLabel: string, name: string) {
  await page.getByRole("button", { name: `New ${typeLabel}` }).click();
  const nameInput = page.getByRole("textbox", { name: "Name" });
  await expect(nameInput).toHaveValue(`New ${typeLabel}`);
  await nameInput.fill(name);
  await expectSaved();
}

test("create world, entities, lore mention, search, pin and event", async () => {
  // 1. Create a world (the folder picker is answered by ORBIS_E2E_DIR).
  await page.getByRole("button", { name: "New world" }).click();
  await page.getByLabel("World name").fill("E2E World");
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Your world is empty" })).toBeVisible();

  // 2. Create entities: a location first (the mention target), then a character.
  await page.getByRole("button", { name: "New location" }).first().click();
  await page.getByRole("textbox", { name: "Name" }).fill("Port Hallow");
  await expectSaved();

  await createEntity("Character", "Aldric the Wanderer");

  // 3. Lore with a [[mention]]: type "[[Port", pick the location from the picker.
  const editor = page.locator(".ProseMirror");
  await editor.click();
  await page.keyboard.type("Aldric was raised in [[Port");
  await page
    .getByRole("option", { name: /Port Hallow/ })
    .first()
    .click();
  await page.keyboard.type(" and carries a lantern.");
  await expect(editor.locator("[data-mention-id]")).toHaveText("Port Hallow");
  await expectSaved();

  // The mention shows up as a backlink on the location.
  await page.getByRole("button", { name: "Port Hallow" }).first().click();
  await expect(page.getByRole("heading", { name: "Mentioned in (1)" }).first()).toBeVisible();
  await expect(page.getByText(/raised in/).first()).toBeVisible();

  // 4. Search: the palette finds the character by a word from its lore.
  await page.keyboard.press("Control+k");
  await page.getByPlaceholder(/Search entities/).fill("lantern");
  const hit = page.getByRole("option", { name: /Aldric the Wanderer/ });
  await expect(hit).toBeVisible();
  await hit.click();
  await expect(page.getByRole("textbox", { name: "Name" })).toHaveValue("Aldric the Wanderer");

  // 5. Map: create one, import the image, place a pin on Port Hallow.
  // createEntity waits for the new page before typing; filling straight away
  // could rename whichever entity was still open.
  await createEntity("Map", "Shattered Coast");
  await page.getByRole("button", { name: "Import map image" }).click();
  const canvas = page.locator("canvas").first();
  await expect(canvas).toBeVisible();
  await page.getByRole("button", { name: /^Pin/ }).click();
  const box = await canvas.boundingBox();
  if (!box) throw new Error("map canvas has no bounding box");
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await page.getByPlaceholder(/Search entities, or type a name/).fill("Port");
  await page
    .getByRole("option", { name: /Port Hallow/ })
    .first()
    .click();
  // The picker closes once the pin has been created for the chosen entity.
  await expect(page.getByPlaceholder(/Search entities, or type a name/)).toHaveCount(0);
  await expect(page.getByText("No pins yet")).toHaveCount(0);

  // The location now lists the map under "appears on maps". Scope the check
  // to that panel: the map's name also appears (sometimes hidden) in the
  // sidebar and tab bar, so a page-wide text match is unreliable.
  await page.getByRole("button", { name: "Port Hallow" }).first().click();
  await expect(page.getByRole("textbox", { name: "Name" })).toHaveValue("Port Hallow");
  const appearsOn = page.getByRole("region", { name: "Appears on maps" });
  await expect(appearsOn.getByRole("heading")).toHaveText("Appears on maps (1)");
  await expect(appearsOn.getByRole("button", { name: /Shattered Coast/ })).toBeVisible();

  // 6. Event: double-click the timeline to create one.
  await page.getByRole("button", { name: "Timeline" }).click();
  await page.getByRole("application").dblclick();
  await page.getByLabel("Event name").fill("The Founding");
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await expect(page.getByRole("textbox", { name: "Name" })).toHaveValue("The Founding");
  await expect(page.getByRole("button", { name: "The Founding" }).first()).toBeVisible();

  // 7. Relationship: Aldric is located in Port Hallow, then show it in the graph.
  await page.getByRole("button", { name: "Aldric the Wanderer" }).first().click();
  await expect(page.getByRole("textbox", { name: "Name" })).toHaveValue("Aldric the Wanderer");
  await page.getByRole("button", { name: "Add relationship" }).click();
  await page.getByLabel("Relationship", { exact: true }).selectOption({ label: "located in" });
  await page.getByRole("button", { name: "Choose entity" }).click();
  await page.getByPlaceholder(/Search entities, or type a name/).fill("Port");
  await page
    .getByRole("option", { name: /Port Hallow/ })
    .first()
    .click();
  await page.getByRole("button", { name: "Add", exact: true }).click();
  const relationships = page.getByRole("region", { name: "Relationships" });
  await expect(
    relationships.getByRole("button", { name: "Port Hallow", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Show in graph" }).click();
  await expect(page.locator(".react-flow__node")).toHaveCount(2);
  await expect(page.locator(".react-flow__node", { hasText: "Port Hallow" })).toBeVisible();

  // Tidy up: leave the world and drop it (and any leftovers from earlier,
  // failed runs) from the recent worlds list, which lives outside the temp dir.
  await page.getByRole("button", { name: "Close world" }).click();
  const remove = page.getByRole("button", { name: "Remove E2E World from recent worlds" });
  await expect(remove.first()).toBeVisible();
  for (let left = await remove.count(); left > 0; left--) {
    await remove.first().click();
    await expect(remove).toHaveCount(left - 1);
  }
});
