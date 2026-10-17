import { useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { FolderOpen, FolderPlus, Sparkles, Trash2 } from "lucide-react";
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

/** Landing screen: create a world, open one, or pick a recent one. */
export function WelcomePage() {
  const navigate = useNavigate();
  const appInfo = useAppInfo();
  const recent = useRecentWorlds();
  const createWorld = useCreateWorld();
  const createSample = useCreateSampleWorld();
  const openWorld = useOpenWorld();
  const removeRecent = useRemoveRecentWorld();

  // Folder + name for the "New world" dialog; null = dialog closed.
  const [draft, setDraft] = useState<{ path: string; name: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const busy = createWorld.isPending || openWorld.isPending || createSample.isPending;

  async function handleNew() {
    const path = await pickFolder("Choose an empty folder for the new world");
    if (path) setDraft({ path, name: folderName(path) });
  }

  async function handleCreate() {
    if (!draft) return;
    setError(null);
    try {
      await createWorld.mutateAsync(draft);
      setDraft(null);
      await navigate({ to: "/world" });
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

        <div className="flex gap-3">
          <Button onClick={handleNew} disabled={busy}>
            <FolderPlus /> New world
          </Button>
          <Button variant="outline" onClick={() => handleOpen()} disabled={busy}>
            <FolderOpen /> Open world
          </Button>
          <Button variant="outline" onClick={handleSample} disabled={busy}>
            <Sparkles /> Create sample world
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
            <DialogTitle>New world</DialogTitle>
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
              Create
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
