import { mkdirSync, readdirSync, readFileSync, existsSync } from "node:fs";
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

async function expectSaved() {
  await expect(page.getByRole("status").filter({ hasText: /^Saved$/ })).toBeVisible();
}

test("custom type with fields, markdown export and print view", async () => {
  // A fresh world (the folder picker is answered by ORBIS_E2E_DIR).
  await page.getByRole("button", { name: "New world" }).click();
  await page.getByLabel("World name").fill("Export World");
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Your world is empty" })).toBeVisible();

  // 1. Create the "Deity" type with a select and an entity reference field.
  await page.getByRole("button", { name: "Settings and debug menu" }).click();
  await page.getByRole("menuitem", { name: "Entity types" }).click();
  await page.getByRole("button", { name: "New type" }).click();
  await page.getByLabel("Name (singular)").fill("Deity");
  await expect(page.getByLabel("Plural")).toHaveValue("Deitys");
  await page.getByLabel("Plural").fill("Deities");

  await page.getByRole("button", { name: "Add field" }).click();
  await page.getByLabel("Label of field 1").fill("Domain");
  await page.getByLabel("Kind of Domain").selectOption("select");
  await page.getByRole("button", { name: "Add option" }).click();
  await page.getByLabel("Option 1 of Domain", { exact: true }).fill("Sea");
  await page.getByRole("button", { name: "Add option" }).click();
  await page.getByLabel("Option 2 of Domain", { exact: true }).fill("War");

  await page.getByRole("button", { name: "Add field" }).click();
  await page.getByLabel("Label of field 2").fill("Worshippers");
  await page.getByLabel("Kind of Worshippers").selectOption("entityRef");
  await page.getByLabel("Allow several references").check();
  await page.getByLabel("Factions", { exact: true }).check();

  await page.getByRole("button", { name: "Create type" }).click();
  await expect(page.getByText("Editing Deity")).toBeVisible();

  // 2. A faction to worship the deity, then the deity itself.
  await page.getByRole("button", { name: "New Faction" }).click();
  const factionName = page.getByRole("textbox", { name: "Name" });
  await expect(factionName).toHaveValue("New Faction");
  await factionName.fill("Tidewardens");
  await expect(page.getByRole("button", { name: "Tidewardens" }).first()).toBeVisible();
  await expectSaved();

  await page.getByRole("button", { name: "New Deity" }).click();
  const name = page.getByRole("textbox", { name: "Name" });
  await expect(name).toHaveValue("New Deity");
  await name.fill("Aelith");
  await page.getByLabel("Domain", { exact: true }).selectOption("Sea");
  await page.getByRole("button", { name: "Add…" }).click();
  await page.getByRole("option", { name: /Tidewardens/ }).click();
  await expect(page.locator("#field-worshippers").getByText("Tidewardens")).toBeVisible();
  await expectSaved();

  // 3. Export to Markdown into <e2eDir>/export-md (the folder must exist).
  const target = path.join(app.dir, "export-md");
  mkdirSync(target, { recursive: true });
  await page.getByRole("button", { name: "Settings and debug menu" }).click();
  await page.getByRole("menuitem", { name: "Export to Markdown…" }).click();
  const dialog = page.getByRole("dialog", { name: "Export to Markdown" });
  await dialog.getByRole("button", { name: /Choose folder/ }).click();
  await expect(dialog.getByText(/export-md/)).toBeVisible();
  await dialog.getByRole("button", { name: "Export", exact: true }).click();
  await expect(dialog.getByRole("status")).toContainText(/Exported \d+ files/, {
    timeout: 30_000,
  });

  const out = path.join(target, "Export World");
  expect(existsSync(path.join(out, "README.md"))).toBe(true);
  const deities = path.join(out, "Deities");
  expect(existsSync(deities)).toBe(true);
  const notes = readdirSync(deities).filter((f) => f.endsWith(".md"));
  expect(notes.length).toBe(1);
  const note = readFileSync(path.join(deities, notes[0]!), "utf8");
  expect(note.startsWith("---")).toBe(true);
  expect(note).toContain("Sea");
  expect(note).toMatch(/\[\[[^\]]*Tidewardens[^\]]*\]\]/);
  await dialog.getByRole("button", { name: "Close" }).first().click();

  // 4. Print view: rendered outside the app shell with the entity's content.
  await page.getByRole("button", { name: "Aelith" }).first().click();
  await page.getByRole("button", { name: "Print or save as PDF" }).click();
  await expect(page).toHaveURL(/#\/print\/entity\//);
  const root = page.locator(".print-root");
  await expect(root).toHaveAttribute("data-print-ready", "true");
  await expect(root.getByRole("heading", { level: 1, name: "Aelith" })).toBeVisible();
  await expect(root.getByText("Sea", { exact: true })).toBeVisible();
  await expect(root.getByText("Tidewardens")).toBeVisible();
  // No app chrome here.
  await expect(page.getByRole("button", { name: "Close world" })).toHaveCount(0);
  await page.getByRole("button", { name: "Back" }).click();
  await expect(page.getByRole("textbox", { name: "Name" })).toHaveValue("Aelith");

  // The whole-world print view lists every entity.
  await page.getByRole("button", { name: "Settings and debug menu" }).click();
  await page.getByRole("menuitem", { name: "Print world…" }).click();
  await expect(page.locator(".print-root")).toHaveAttribute("data-print-ready", "true");
  await expect(page.getByRole("region", { name: "Contents" })).toContainText("Aelith");
  await expect(page.getByRole("region", { name: "Contents" })).toContainText("Tidewardens");
});
