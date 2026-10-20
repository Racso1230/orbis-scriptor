import { useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { FileUp, FolderOpen, FolderPlus, Mountain, Settings, Sparkles, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { useAppInfo } from "@/features/app/queries";
import { useGenerateStore } from "@/features/generator/generate-store";
import { useImportStore } from "@/features/importer/import-store";
import { folderName, pickFolder } from "./pick-folder";
import {
  useCreateSampleWorld,
  useCreateWorld,
  useOpenWorld,
  useRecentWorlds,
  useRemoveRecentWorld,
} from "./queries";

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** What to open after "New world" has created and opened the world. */
type NextStep = "import" | "generate" | null;

const NEW_WORLD_TITLES: Record<"import" | "generate", string> = {
  import: "New world for the import",
  generate: "New world for the generated map",
};

/** Landing screen: create a world, open one, or pick a recent one. */
export function WelcomePage() {
  const navigate = useNavigate();
  const appInfo = useAppInfo();
  const recent = useRecentWorlds();
  const createWorld = useCreateWorld();
  const createSample = useCreateSampleWorld();
  const openWorld = useOpenWorld();
  const removeRecent = useRemoveRecentWorld();

  // Folder + name for the "New world" dialog; null = dialog closed. `then` is the
  // wizard to open once the new world is shown.
  const [draft, setDraft] = useState<{ path: string; name: string; then: NextStep } | null>(null);
  const openImport = useImportStore((s) => s.openDialog);
  const openGenerate = useGenerateStore((s) => s.openDialog);
  const [error, setError] = useState<string | null>(null);
  const busy = createWorld.isPending || openWorld.isPending || createSample.isPending;

  async function handleNew(then: NextStep = null) {
    const path = await pickFolder("Choose an empty folder for the new world");
    if (path) setDraft({ path, name: folderName(path), then });
  }

  async function handleCreate() {
    if (!draft) return;
    setError(null);
    try {
      await createWorld.mutateAsync({ path: draft.path, name: draft.name });
      const then = draft.then;
      setDraft(null);
      await navigate({ to: "/world" });
      // The wizards live in the world layout, so they open once the world is shown.
      if (then === "import") openImport();
      if (then === "generate") openGenerate();
    } catch (e) {
      setError(messageOf(e));
    }
  }

  async function handleSample() {
    const parent = await pickFolder('Choose where to create "The Shattered Coast"');
    if (!parent) return;
    setError(null);
    try {
      await createSample.mutateAsync(parent);
      await navigate({ to: "/world" });
    } catch (e) {
      setError(messageOf(e));
    }
  }

  async function handleOpen(path?: string) {
    const target = path ?? (await pickFolder("Choose a world folder"));
    if (!target) return;
    setError(null);
    try {
      await openWorld.mutateAsync(target);
      await navigate({ to: "/world" });
    } catch (e) {
      setError(messageOf(e));
    }
  }

  return (
    <div className="flex flex-1 items-center justify-center p-8">
      <div className="flex w-full max-w-xl flex-col gap-6">
        <div>
          <h1 className="text-2xl font-semibold">Orbis Scriptor</h1>
          <p className="text-muted-foreground">Local-first worldbuilding</p>
        </div>

        <div className="flex flex-wrap gap-3">
          <Button onClick={() => handleNew()} disabled={busy}>
            <FolderPlus /> New world
          </Button>
          <Button variant="outline" onClick={() => handleOpen()} disabled={busy}>
            <FolderOpen /> Open world
          </Button>
          <Button variant="outline" onClick={handleSample} disabled={busy}>
            <Sparkles /> Create sample world
          </Button>
          <Button variant="outline" onClick={() => handleNew("import")} disabled={busy}>
            <FileUp /> Import vault…
          </Button>
          <Button variant="outline" onClick={() => handleNew("generate")} disabled={busy}>
            <Mountain /> Generate world…
          </Button>
          <Button
            variant="ghost"
            className="ml-auto"
            onClick={() => void navigate({ to: "/settings" })}
          >
            <Settings /> Settings
          </Button>
        </div>

        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}

        <Separator />

        <section className="flex flex-col gap-2">
          <h2 className="text-sm font-medium text-muted-foreground">Recent worlds</h2>
          {recent.data?.length === 0 && (
            <p className="text-sm text-muted-foreground">No recent worlds yet.</p>
          )}
          <ul className="flex flex-col gap-1">
            {recent.data?.map((world) => (
              <li key={world.path} className="flex items-center gap-2">
                <button
                  type="button"
                  className="flex min-w-0 flex-1 flex-col rounded-md px-3 py-2 text-left hover:bg-accent disabled:cursor-not-allowed disabled:opacity-50"
                  disabled={!world.available || busy}
                  onClick={() => handleOpen(world.path)}
                  title={world.available ? world.path : "This folder is no longer available"}
                >
                  <span className="truncate font-medium">
                    {world.name}
                    {!world.available && (
                      <span className="ml-2 text-xs font-normal text-muted-foreground">
                        (unavailable)
                      </span>
                    )}
                  </span>
                  <span className="truncate text-xs text-muted-foreground">{world.path}</span>
                </button>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label={`Remove ${world.name} from recent worlds`}
                  onClick={() => removeRecent.mutate(world.path)}
                >
                  <Trash2 />
                </Button>
              </li>
            ))}
          </ul>
        </section>

        {appInfo.data && (
          <p className="text-xs text-muted-foreground">
            {appInfo.data.name} {appInfo.data.version}
          </p>
        )}
      </div>

      <Dialog open={draft !== null} onOpenChange={(open) => !open && setDraft(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{draft?.then ? NEW_WORLD_TITLES[draft.then] : "New world"}</DialogTitle>
            <DialogDescription className="truncate">{draft?.path}</DialogDescription>
          </DialogHeader>
          <div className="flex flex-col gap-2">
            <Label htmlFor="world-name">World name</Label>
            <Input
              id="world-name"
              autoFocus
              value={draft?.name ?? ""}
              onChange={(e) => draft && setDraft({ ...draft, name: e.target.value })}
              onKeyDown={(e) => e.key === "Enter" && handleCreate()}
            />
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDraft(null)} disabled={busy}>
              Cancel
            </Button>
            <Button onClick={handleCreate} disabled={busy || !draft?.name.trim()}>
              {draft?.then ? "Create and continue" : "Create"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
