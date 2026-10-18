import { useMemo, useState } from "react";
import { FolderOpen } from "lucide-react";
import { NativeSelect } from "@/components/native-select";
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
import { useEntityTypes } from "@/features/entities/queries";
import { IconPicker, ColorPicker } from "@/features/types/pickers";
import { useTypeIcons } from "@/features/types/queries";
import type { ImportOptions, ImportPreview } from "@/lib/bindings";
import { useDebouncedValue } from "@/lib/use-debounced-value";
import { IMPORT_PHASE_LABELS, importFraction } from "./import-state";
import { startVaultImport, useImportStore } from "./import-store";
import { pickVaultFolder } from "./pick-vault-folder";
import { useImportPreview } from "./queries";
import {
  buildFolderTypeMap,
  choiceToSelectValue,
  customChoiceFor,
  customChoiceProblem,
  DEFAULT_CHOICE,
  firstChoiceProblem,
  type FolderChoice,
} from "./wizard-model";

/** Mounted once in the world layout; opened from the gear menu, the palette and the welcome page. */
export function ImportWizard() {
  const open = useImportStore((s) => s.dialogOpen);
  const setOpen = useImportStore((s) => s.setDialogOpen);
  const openCount = useImportStore((s) => s.openCount);
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent className="max-h-[88vh] overflow-y-auto sm:max-w-2xl">
        {/* A fresh body (step one, no folder) every time the wizard is opened anew. */}
        <WizardBody key={openCount} onClose={() => setOpen(false)} />
      </DialogContent>
    </Dialog>
  );
}

type Step = 1 | 2 | 3;
const STEP_TITLES: Record<Step, string> = {
  1: "Choose the vault folder",
  2: "Options",
  3: "Preview and folder mapping",
};

function WizardBody({ onClose }: { onClose: () => void }) {
  const run = useImportStore((s) => s.state);
  const types = useEntityTypes();
  const typeList = types.data ?? [];

  const [step, setStep] = useState<Step>(1);
  const [dir, setDir] = useState<string | null>(null);
  const [defaultType, setDefaultType] = useState<string | null>(null);
  const [skipDuplicates, setSkipDuplicates] = useState(true);
  const [createMissingLinksAsNotes, setCreateMissing] = useState(false);
  const [importImages, setImportImages] = useState(true);
  const [choices, setChoices] = useState<Record<string, FolderChoice>>({});

  const effectiveDefault =
    defaultType ?? (typeList.some((t) => t.key === "note") ? "note" : (typeList[0]?.key ?? "note"));

  const baseOptions: ImportOptions = {
    defaultType: effectiveDefault,
    folderTypeMap: [],
    createMissingLinksAsNotes,
    importImages,
    skipDuplicates,
  };

  // Folders come from a preview WITHOUT mappings, so a folder you map does not vanish from the list.
  const inPreview = step === 3 && run.status === "idle";
  const debouncedBase = useDebouncedValue(baseOptions, 250);
  const base = useImportPreview(inPreview ? dir : null, debouncedBase);
  const folders = useMemo(
    () => (base.data?.unresolvedTypes ?? []).map((f) => f.folder),
    [base.data],
  );

  const options: ImportOptions = {
    ...baseOptions,
    folderTypeMap: buildFolderTypeMap(folders, choices),
  };
  const debouncedOptions = useDebouncedValue(options, 250);
  const preview = useImportPreview(inPreview ? dir : null, debouncedOptions);
  const problem = firstChoiceProblem(folders, choices);

  async function choose() {
    const picked = await pickVaultFolder();
    if (picked) {
      setDir(picked);
      setChoices({});
    }
  }

  if (run.status !== "idle") {
    return (
      <RunView
        onClose={onClose}
        onBack={() => useImportStore.getState().dispatch({ type: "reset" })}
      />
    );
  }

  return (
    <>
      <DialogHeader>
        <DialogTitle>Import Markdown vault</DialogTitle>
        <DialogDescription>
          Step {step} of 3: {STEP_TITLES[step]}. Notes become entities; nothing in the vault is
          changed.
        </DialogDescription>
      </DialogHeader>

      {step === 1 && (
        <div className="flex flex-col gap-2">
          <p className="text-sm text-muted-foreground">
            Pick a folder of .md notes: an Obsidian vault, a plain Markdown folder, or an Orbis
            Scriptor export.
          </p>
          <div className="flex items-center gap-2">
            <Button variant="outline" size="sm" onClick={() => void choose()}>
              <FolderOpen /> Choose vault folder…
            </Button>
            <span className="min-w-0 truncate text-sm text-muted-foreground" title={dir ?? ""}>
              {dir ?? "No folder chosen"}
            </span>
          </div>
        </div>
      )}

      {step === 2 && (
        <fieldset className="flex flex-col gap-3">
          <legend className="sr-only">Import options</legend>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="import-default-type">Type for notes without a clear type</Label>
            <NativeSelect
              id="import-default-type"
              value={effectiveDefault}
              onChange={(e) => setDefaultType(e.target.value)}
              className="max-w-xs"
            >
              {typeList.map((t) => (
                <option key={t.key} value={t.key}>
                  {t.label}
                </option>
              ))}
            </NativeSelect>
          </div>
          <Check
            label="Skip notes that already exist (same type and name)"
            checked={skipDuplicates}
            onChange={setSkipDuplicates}
          />
          <Check
            label="Turn links to missing notes into empty Note entities"
            checked={createMissingLinksAsNotes}
            onChange={setCreateMissing}
          />
          <Check
            label="Import images into the entities' galleries"
            checked={importImages}
            onChange={setImportImages}
          />
        </fieldset>
      )}

      {step === 3 && (
        <PreviewView
          preview={preview.data}
          loading={preview.isFetching || base.isFetching}
          error={preview.error?.message ?? base.error?.message ?? null}
          typeList={typeList}
          folders={base.data?.unresolvedTypes ?? []}
          choices={choices}
          onChoice={(folder, choice) => setChoices((c) => ({ ...c, [folder]: choice }))}
          defaultTypeLabel={typeList.find((t) => t.key === effectiveDefault)?.label ?? "Note"}
        />
      )}

      <DialogFooter>
        <Button variant="outline" onClick={onClose}>
          Close
        </Button>
        {step > 1 && (
          <Button variant="outline" onClick={() => setStep((step - 1) as Step)}>
            Back
          </Button>
        )}
        {step < 3 && (
          <Button disabled={dir === null} onClick={() => setStep((step + 1) as Step)}>
            Next
          </Button>
        )}
        {step === 3 && (
          <Button
            disabled={
              dir === null ||
              preview.data === undefined ||
              preview.data.notes + preview.data.skipped === 0 ||
              problem !== null ||
              preview.error !== null
            }
            title={problem ?? undefined}
            onClick={() => dir && void startVaultImport(dir, options)}
          >
            Import
          </Button>
        )}
      </DialogFooter>
    </>
  );
}

function Check({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (on: boolean) => void;
}) {
  return (
    <label className="flex items-center gap-2 text-sm">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      {label}
    </label>
  );
}

function PreviewView({
  preview,
  loading,
  error,
  typeList,
  folders,
  choices,
  onChoice,
  defaultTypeLabel,
}: {
  preview: ImportPreview | undefined;
  loading: boolean;
  error: string | null;
  typeList: { key: string; label: string }[];
  folders: { folder: string; count: number }[];
  choices: Record<string, FolderChoice>;
  onChoice: (folder: string, choice: FolderChoice) => void;
  defaultTypeLabel: string;
}) {
  if (error) {
    return (
      <p role="alert" className="text-sm text-destructive">
        {error}
      </p>
    );
  }
  if (!preview) return <p className="text-sm text-muted-foreground">Reading the vault…</p>;

  return (
    <div className="flex flex-col gap-4" aria-busy={loading}>
      <p className="text-sm" role="status">
        <strong>{preview.notes}</strong> {preview.notes === 1 ? "note" : "notes"} will be imported
        {preview.skipped > 0 && `, ${preview.skipped} skipped as duplicates`}.
        {preview.orbisExport && " This looks like an Orbis Scriptor export."}
        {loading && <span className="ml-2 text-muted-foreground">Updating…</span>}
      </p>

      <ul className="flex flex-wrap gap-2" aria-label="Notes per type">
        {preview.byType.map((t) => (
          <li key={t.typeKey}>
            <Badge variant="secondary">
              {t.label}: {t.count}
              {t.newType && " (new type)"}
            </Badge>
          </li>
        ))}
      </ul>

      <dl className="grid grid-cols-4 gap-2 text-sm">
        <Stat label="Images" value={preview.images} />
        <Stat label="Tags" value={preview.tags} />
        <Stat label="Relationships" value={preview.relationships} />
        <Stat label="Skipped" value={preview.skipped} />
      </dl>

      {folders.length > 0 && (
        <section aria-label="Folder mapping" className="flex flex-col gap-3">
          <div>
            <h3 className="text-sm font-medium">Folders without a matching type</h3>
            <p className="text-xs text-muted-foreground">
              Choose what the notes of each folder become. By default they use {defaultTypeLabel}.
            </p>
          </div>
          {folders.map((f) => (
            <FolderMapping
              key={f.folder}
              folder={f.folder}
              count={f.count}
              typeList={typeList}
              choice={choices[f.folder] ?? DEFAULT_CHOICE}
              onChange={(c) => onChoice(f.folder, c)}
              defaultTypeLabel={defaultTypeLabel}
            />
          ))}
        </section>
      )}

      {preview.warnings.length > 0 && (
        <section aria-label="Warnings" className="flex flex-col gap-1">
          <h3 className="text-sm font-medium">Warnings</h3>
          <ul className="max-h-32 list-disc overflow-auto pl-5 text-xs text-muted-foreground">
            {preview.warnings.map((w, i) => (
              <li key={i}>{w}</li>
            ))}
          </ul>
          {preview.moreWarnings > 0 && (
            <p className="text-xs text-muted-foreground">and {preview.moreWarnings} more</p>
          )}
        </section>
      )}
    </div>
  );
}

function Stat({ label, value }: { label: string; value: number }) {
  return (
    <div className="rounded-md border px-2 py-1">
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className="font-medium tabular-nums">{value}</dd>
    </div>
  );
}

function FolderMapping({
  folder,
  count,
  typeList,
  choice,
  onChange,
  defaultTypeLabel,
}: {
  folder: string;
  count: number;
  typeList: { key: string; label: string }[];
  choice: FolderChoice;
  onChange: (choice: FolderChoice) => void;
  defaultTypeLabel: string;
}) {
  const icons = useTypeIcons();
  const problem = customChoiceProblem(choice);

  function select(value: string) {
    if (value === "default") onChange(DEFAULT_CHOICE);
    else if (value === "custom") {
      onChange(choice.mode === "custom" ? choice : customChoiceFor(folder));
    } else onChange({ mode: "type", typeKey: value.slice("type:".length) });
  }

  return (
    <div className="flex flex-col gap-2 rounded-lg border p-3">
      <div className="flex items-center gap-3">
        <Label htmlFor={`map-${folder}`} className="min-w-0 flex-1 truncate">
          {folder}/ <span className="font-normal text-muted-foreground">({count} notes)</span>
        </Label>
        <NativeSelect
          id={`map-${folder}`}
          aria-label={`Type for folder ${folder}`}
          value={choiceToSelectValue(choice)}
          onChange={(e) => select(e.target.value)}
        >
          <option value="default">Default ({defaultTypeLabel})</option>
          {typeList.map((t) => (
            <option key={t.key} value={`type:${t.key}`}>
              {t.label}
            </option>
          ))}
          <option value="custom">Create custom type…</option>
        </NativeSelect>
      </div>
      {choice.mode === "custom" && (
        <div className="flex flex-col gap-3 border-t pt-3">
          <div className="grid grid-cols-2 gap-3">
            <div className="flex flex-col gap-1">
              <Label htmlFor={`label-${folder}`}>Name (singular)</Label>
              <Input
                id={`label-${folder}`}
                value={choice.label}
                onChange={(e) => onChange({ ...choice, label: e.target.value })}
              />
            </div>
            <div className="flex flex-col gap-1">
              <Label htmlFor={`plural-${folder}`}>Plural</Label>
              <Input
                id={`plural-${folder}`}
                value={choice.plural}
                onChange={(e) => onChange({ ...choice, plural: e.target.value })}
              />
            </div>
          </div>
          <IconPicker
            icons={icons.data ?? []}
            value={choice.icon}
            color={choice.color}
            onChange={(icon) => onChange({ ...choice, icon })}
          />
          <ColorPicker value={choice.color} onChange={(color) => onChange({ ...choice, color })} />
          {problem && (
            <p role="alert" className="text-xs text-destructive">
              {problem}
            </p>
          )}
        </div>
      )}
    </div>
  );
}

function RunView({ onClose, onBack }: { onClose: () => void; onBack: () => void }) {
  const run = useImportStore((s) => s.state);
  const fraction = importFraction(run);
  const running = run.status === "running";

  return (
    <>
      <DialogHeader>
        <DialogTitle>Import Markdown vault</DialogTitle>
        <DialogDescription>
          {running
            ? "Importing. You can hide this window; the import keeps running."
            : run.status === "done"
              ? "The import is complete."
              : "The import did not finish."}
        </DialogDescription>
      </DialogHeader>

      <div className="flex flex-col gap-2" aria-live="polite">
        {running && (
          <>
            <div
              role="progressbar"
              aria-label="Import progress"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={fraction === null ? undefined : Math.round(fraction * 100)}
              className="h-2 w-full overflow-hidden rounded-full bg-muted"
            >
              <div
                className={
                  fraction === null
                    ? "h-full w-1/3 animate-pulse bg-primary"
                    : "h-full bg-primary transition-all"
                }
                style={fraction === null ? undefined : { width: `${fraction * 100}%` }}
              />
            </div>
            <p className="text-sm text-muted-foreground">
              {IMPORT_PHASE_LABELS[run.phase] ?? run.phase}
              {run.total > 0 && ` ${run.done} / ${run.total}`}
            </p>
          </>
        )}
        {run.status === "done" && run.result && (
          <>
            <p role="status" className="text-sm">
              Imported {run.result.created} {run.result.created === 1 ? "entity" : "entities"}
              {run.result.skipped > 0 && `, skipped ${run.result.skipped}`}.
            </p>
            <dl className="grid grid-cols-3 gap-2 text-sm">
              <Stat label="Images" value={run.result.images} />
              <Stat label="Relationships" value={run.result.relationships} />
              <Stat label="New types" value={run.result.typesCreated} />
            </dl>
            {run.result.warnings.length > 0 && (
              <section aria-label="Import warnings" className="flex flex-col gap-1">
                <h3 className="text-sm font-medium">Warnings</h3>
                <ul className="max-h-40 list-disc overflow-auto pl-5 text-xs text-muted-foreground">
                  {run.result.warnings.map((w, i) => (
                    <li key={i}>{w}</li>
                  ))}
                </ul>
                {run.result.moreWarnings > 0 && (
                  <p className="text-xs text-muted-foreground">
                    and {run.result.moreWarnings} more
                  </p>
                )}
              </section>
            )}
          </>
        )}
        {run.status === "failed" && (
          <p role="alert" className="text-sm text-destructive">
            {run.error} Nothing was imported.
          </p>
        )}
      </div>

      <DialogFooter>
        {run.status === "failed" && (
          <Button variant="outline" onClick={onBack}>
            Start over
          </Button>
        )}
        <Button variant={running ? "outline" : "default"} onClick={onClose}>
          {running ? "Hide" : "Close"}
        </Button>
      </DialogFooter>
    </>
  );
}
