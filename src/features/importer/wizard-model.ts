import { DEFAULT_COLOR, isHexColor } from "@/features/types/draft";
import type { FolderTypeMapping } from "@/lib/bindings";

/** What the user chose for one folder that matched no entity type. */
export type FolderChoice =
  | { mode: "default" }
  | { mode: "type"; typeKey: string }
  | { mode: "custom"; label: string; plural: string; icon: string; color: string };

export const DEFAULT_CHOICE: FolderChoice = { mode: "default" };

/** The starting values of the "create a custom type" form for a folder. */
export function customChoiceFor(folder: string): Extract<FolderChoice, { mode: "custom" }> {
  return { mode: "custom", label: folder, plural: folder, icon: "box", color: DEFAULT_COLOR };
}

const MAX_NAME = 40;

/** A problem with a custom type form, in plain words, or `null` when it is fine. */
export function customChoiceProblem(choice: FolderChoice): string | null {
  if (choice.mode !== "custom") return null;
  if (choice.label.trim() === "") return "Give the new type a name.";
  if (choice.plural.trim() === "") return "Give the new type a plural name.";
  if (choice.label.trim().length > MAX_NAME || choice.plural.trim().length > MAX_NAME) {
    return `Type names can have at most ${MAX_NAME} characters.`;
  }
  if (!isHexColor(choice.color)) return "Use a colour like #2563eb.";
  return null;
}

/** First problem among all folders, or `null`: the Import button waits for this. */
export function firstChoiceProblem(
  folders: readonly string[],
  choices: Readonly<Record<string, FolderChoice>>,
): string | null {
  for (const folder of folders) {
    const problem = customChoiceProblem(choices[folder] ?? DEFAULT_CHOICE);
    if (problem) return `${folder}: ${problem}`;
  }
  return null;
}

/**
 * The `folderTypeMap` the backend expects: one entry per folder the user mapped
 * (exactly one of `typeKey` / `createCustom`). Folders left on "default" are omitted.
 */
export function buildFolderTypeMap(
  folders: readonly string[],
  choices: Readonly<Record<string, FolderChoice>>,
): FolderTypeMapping[] {
  const out: FolderTypeMapping[] = [];
  for (const folder of folders) {
    const choice = choices[folder] ?? DEFAULT_CHOICE;
    if (choice.mode === "type") {
      out.push({ folder, typeKey: choice.typeKey, createCustom: null });
    } else if (choice.mode === "custom") {
      out.push({
        folder,
        typeKey: null,
        createCustom: {
          label: choice.label.trim(),
          plural: choice.plural.trim(),
          icon: choice.icon,
          color: choice.color.toLowerCase(),
        },
      });
    }
  }
  return out;
}

/** Value of the folder select for a choice (`default`, `custom` or `type:<key>`). */
export function choiceToSelectValue(choice: FolderChoice): string {
  if (choice.mode === "type") return `type:${choice.typeKey}`;
  return choice.mode;
}
