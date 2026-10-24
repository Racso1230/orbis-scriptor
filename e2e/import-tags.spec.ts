import { copyFileSync, mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
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

/** A small hand-made Obsidian vault: nested folders, front matter, #tags, [[links]] and an image. */
function writeVault(dir: string) {
  const vault = path.join(dir, "vault");
  mkdirSync(path.join(vault, "Characters"), { recursive: true });
  mkdirSync(path.join(vault, "Places", "Cities"), { recursive: true });
  mkdirSync(path.join(vault, "attachments"), { recursive: true });
  copyFileSync(path.join(dir, "map.png"), path.join(vault, "attachments", "img.png"));
  writeFileSync(
    path.join(vault, "Characters", "Aldric.md"),
    [
      "---",
      "summary: A wandering sailor.",
      "tags: [hero, places/sailors]",
      "---",
      "# Aldric",
      "",
      "Raised in [[Port Calder]] and fond of #sailing.",
      "",
      "![[img.png]]",
      "",
    ].join("\n"),
  );
  writeFileSync(
    path.join(vault, "Places", "Cities", "Port Calder.md"),
    ["---", "tags: [harbour]", "---", "A busy harbour town.", ""].join("\n"),
  );
}

test("import a vault, then use tags, favourites, gallery and portraits", async () => {
  // A fresh world; the vault goes to <e2eDir>/vault (answered by the folder hook).
  await page.getByRole("button", { name: "New world" }).click();
  await page.getByLabel("World name").fill("Import World");
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Your world is empty" })).toBeVisible();
  writeVault(app.dir);

  // 1. Import wizard: folder, options, preview with folder mapping, import.
  await page.getByRole("button", { name: "Settings and debug menu" }).click();
  await page.getByRole("menuitem", { name: "Import Markdown / Obsidian vault…" }).click();
  const wizard = page.getByRole("dialog", { name: "Import Markdown vault" });
  await wizard.getByRole("button", { name: /Choose vault folder/ }).click();
  // The chosen folder path ends in "/vault" (not the dialog title, which also ends in "vault").
  await expect(wizard.getByText(/[\\/]vault$/)).toBeVisible();
  await wizard.getByRole("button", { name: "Next" }).click();
  await wizard.getByRole("button", { name: "Next" }).click();
  await expect(wizard.getByRole("status")).toContainText("2 notes will be imported");

  // "Cities" matches no type: send those notes to Location.
  await wizard.getByLabel("Type for folder Cities").selectOption({ label: "Location" });
  await wizard.getByRole("button", { name: "Import", exact: true }).click();
  await expect(wizard.getByRole("status")).toContainText("Imported 2 entities");
  await wizard.getByRole("button", { name: "Close" }).first().click();

  // 2. The entities are in the sidebar (under their types).
  const sidebar = page.locator("aside");
  await expect(sidebar.getByRole("button", { name: "Aldric", exact: true })).toBeVisible();
  await expect(sidebar.getByRole("button", { name: "Port Calder", exact: true })).toBeVisible();

  // 3. Aldric has the front matter tags and the inline #tag, and his image is in the gallery.
  await sidebar.getByRole("button", { name: "Aldric", exact: true }).click();
  await expect(page.getByRole("textbox", { name: "Name" })).toHaveValue("Aldric");
  const tags = page.getByRole("list", { name: "Tags" });
  await expect(tags.getByText("hero", { exact: true })).toBeVisible();
  await expect(tags.getByText("sailing", { exact: true })).toBeVisible();
  await expect(tags.getByText("places/sailors", { exact: true })).toBeVisible();
  const gallery = page.getByRole("list", { name: "Gallery images" });
  await expect(gallery.locator("img")).toHaveCount(1);

  // 4. The [[Port Calder]] link became a mention: it shows up in the backlinks.
  await expect(page.locator(".ProseMirror [data-mention-id]")).toHaveText("Port Calder");
  await sidebar.getByRole("button", { name: "Port Calder", exact: true }).first().click();
  await expect(page.getByRole("textbox", { name: "Name" })).toHaveValue("Port Calder");
  await expect(page.getByRole("heading", { name: "Mentioned in (1)" })).toBeVisible();
  await sidebar.getByRole("button", { name: "Aldric", exact: true }).first().click();
  await expect(page.getByRole("textbox", { name: "Name" })).toHaveValue("Aldric");

  // 5. Portrait: set it from the gallery, the avatar appears in the sidebar and the header.
  await expect(sidebar.getByTestId("entity-avatar")).toHaveCount(0);
  await page.getByRole("button", { name: "Set as portrait" }).click();
  await expect(sidebar.getByTestId("entity-avatar").first()).toBeVisible();
  await expect(page.getByRole("button", { name: "Open portrait" })).toBeVisible();

  // Gallery: add a second image (the file dialog is answered by ORBIS_E2E_DIR), caption it.
  await page.getByRole("button", { name: /Add images/ }).click();
  await expect(gallery.locator("img")).toHaveCount(2);
  const caption = page.getByLabel(/^Caption of map\.png/);
  await caption.fill("The coast");
  await caption.press("Enter");
  await expect(page.getByRole("button", { name: "Open The coast" })).toBeVisible();
  // Lightbox: open, step, close.
  await page.getByRole("button", { name: "Open The coast" }).click();
  await expect(page.getByRole("dialog", { name: "Image viewer" })).toBeVisible();
  await page.getByRole("button", { name: "Next image" }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: "Image viewer" })).toBeHidden();

  // 6. Favourite: the star on the page puts Aldric in the Favourites section.
  await page.getByRole("button", { name: "Add to favourites" }).first().click();
  const favourites = sidebar.locator('[data-section="favorites"]');
  await expect(favourites.getByRole("button", { name: "Aldric", exact: true })).toBeVisible();

  // 7. Filter the sidebar by tag.
  await sidebar.getByRole("button", { name: /Filter by tag/ }).click();
  await sidebar
    .getByRole("group", { name: "Tags" })
    .getByRole("button", { name: "harbour" })
    .click();
  await expect(sidebar.getByRole("button", { name: "Port Calder", exact: true })).toBeVisible();
  await expect(sidebar.getByRole("button", { name: "Aldric", exact: true })).toHaveCount(0);
  await sidebar.getByRole("button", { name: "Clear tag filter" }).click();
  // Nested: "places" is not a tag, but places/sailors is; filtering by its parent needs the
  // exact tag in the UI, so check the nested tag chip itself.
  await sidebar
    .getByRole("group", { name: "Tags" })
    .getByRole("button", { name: "places/sailors" })
    .click();
  await expect(sidebar.getByRole("button", { name: "Aldric", exact: true })).toBeVisible();
  await expect(sidebar.getByRole("button", { name: "Port Calder", exact: true })).toHaveCount(0);
  await sidebar.getByRole("button", { name: "Clear tag filter" }).click();

  // 8. Search with a tag filter in the palette.
  await page.keyboard.press("Control+k");
  await page.getByPlaceholder(/Search entities/).fill("tag:harbour");
  await expect(page.getByRole("option", { name: /Port Calder/ })).toBeVisible();
  await expect(page.getByRole("option", { name: /Aldric/ })).toHaveCount(0);
  await page.keyboard.press("Escape");

  // 9. The Tags screen lists the tags with counts.
  await page.getByRole("button", { name: "Settings and debug menu" }).click();
  await page.getByRole("menuitem", { name: "Tags", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Tags", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "harbour 1" })).toBeVisible();
});
