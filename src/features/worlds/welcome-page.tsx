import { useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import {
  Compass,
  FileUp,
  FolderOpen,
  FolderPlus,
  Map as MapIcon,
  Mountain,
  Settings,
  Sparkles,
  Trash2,
} from "lucide-react";
import logoUrl from "@/assets/logo.svg";
import { Badge } from "@/components/ui/badge";
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
    <div className="brand-backdrop relative flex flex-1 flex-col overflow-hidden">
      {/* Decorative compass-rose watermark, bottom-right. */}
      <img
        src={logoUrl}
        alt=""
        aria-hidden="true"
        className="pointer-events-none absolute -right-32 -bottom-32 size-[36rem] opacity-[0.06] select-none dark:opacity-[0.08]"
      />

      <div className="relative flex flex-1 items-center justify-center overflow-y-auto px-6 py-10 sm:px-10">
        <div className="grid w-full max-w-5xl gap-10 md:grid-cols-[minmax(0,1.1fr)_minmax(0,1fr)] md:gap-14">
          {/* Left: brand + actions */}
          <div className="flex flex-col justify-center gap-8">
            <div className="flex items-center gap-5">
              <img
                src={logoUrl}
                alt="Orbis Scriptor"
                data-testid="brand-logo"
                className="size-24 shrink-0 drop-shadow-md md:size-28"
              />
              <div className="flex min-w-0 flex-col gap-2">
                <h1 className="font-brand text-3xl leading-tight font-bold tracking-wide text-brand-navy sm:text-4xl dark:text-brand-navy-foreground">
                  Orbis Scriptor
                </h1>
                <div className="brand-hairline w-full max-w-64" aria-hidden="true" />
                <p className="text-sm tracking-wide text-muted-foreground">
                  Local-first worldbuilding
                </p>
              </div>
            </div>

            <div className="flex flex-col gap-3">
              <div className="flex flex-wrap gap-3">
                <Button
                  size="lg"
                  className="h-10 border border-brand-gold/60 bg-brand-navy px-4 text-brand-navy-foreground shadow-sm hover:bg-brand-navy/90 [&_svg]:text-brand-gold"
                  onClick={() => handleNew()}
                  disabled={busy}
                >
                  <FolderPlus /> New world
                </Button>
                <Button
                  size="lg"
                  variant="outline"
                  className="h-10 px-4"
                  onClick={() => handleOpen()}
                  disabled={busy}
                >
                  <FolderOpen /> Open world
                </Button>
              </div>
              <div className="flex flex-wrap gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => handleNew("generate")}
                  disabled={busy}
                >
                  <Mountain /> Generate world…
                </Button>
                <Button variant="outline" size="sm" onClick={handleSample} disabled={busy}>
                  <Sparkles /> Create sample world
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => handleNew("import")}
                  disabled={busy}
                >
                  <FileUp /> Import vault…
                </Button>
              </div>
            </div>

            {error && (
              <p
                role="alert"
                className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive"
              >
                {error}
              </p>
            )}
          </div>

          {/* Right: recent worlds */}
          <section
            aria-labelledby="recent-worlds-heading"
            className="flex min-w-0 flex-col gap-3 rounded-xl border bg-card/70 p-4 shadow-sm backdrop-blur-sm"
          >
            <h2
              id="recent-worlds-heading"
              className="font-brand px-1 text-sm font-medium tracking-widest text-muted-foreground uppercase"
            >
              Recent worlds
            </h2>
            {recent.data?.length === 0 && (
              <div className="flex flex-col items-center gap-2 px-4 py-8 text-center">
                <Compass className="size-8 text-brand-gold" aria-hidden="true" />
                <p className="text-sm text-muted-foreground">
                  No recent worlds yet. Start a new one, or try the sample world to explore.
                </p>
              </div>
            )}
            <ul className="flex max-h-[22rem] flex-col gap-1 overflow-y-auto">
              {recent.data?.map((world) => (
                <li key={world.path} className="flex items-center gap-1">
                  <button
                    type="button"
                    className="flex min-w-0 flex-1 items-center gap-3 rounded-lg px-3 py-2.5 text-left transition-colors hover:bg-accent focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-60"
                    disabled={!world.available || busy}
                    onClick={() => handleOpen(world.path)}
                    title={world.available ? world.path : "This folder is no longer available"}
                  >
                    <span
                      className="flex size-9 shrink-0 items-center justify-center rounded-full border border-brand-gold/40 bg-brand-navy/10 text-brand-navy dark:bg-brand-navy/40 dark:text-brand-gold"
                      aria-hidden="true"
                    >
                      <MapIcon className="size-4" />
                    </span>
                    <span className="flex min-w-0 flex-1 flex-col">
                      <span className="flex min-w-0 items-center gap-2">
                        <span className="truncate font-medium">{world.name}</span>
                        {!world.available && (
                          <Badge variant="outline" className="shrink-0 text-muted-foreground">
                            unavailable
                          </Badge>
                        )}
                      </span>
                      <span className="truncate text-xs text-muted-foreground">{world.path}</span>
                    </span>
                  </button>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="shrink-0 text-muted-foreground hover:text-destructive"
                    aria-label={`Remove ${world.name} from recent worlds`}
                    onClick={() => removeRecent.mutate(world.path)}
                  >
                    <Trash2 />
                  </Button>
                </li>
              ))}
            </ul>
          </section>
        </div>
      </div>

      <footer className="relative flex items-center justify-between px-6 py-3 text-xs text-muted-foreground sm:px-10">
        <span>{appInfo.data && `${appInfo.data.name} ${appInfo.data.version}`}</span>
        <Button variant="ghost" size="sm" onClick={() => void navigate({ to: "/settings" })}>
          <Settings /> Settings
        </Button>
      </footer>

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
