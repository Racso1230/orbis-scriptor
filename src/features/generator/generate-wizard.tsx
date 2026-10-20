import { useMemo, useState, type ReactNode } from "react";
import { Dices, Loader2, RefreshCw } from "lucide-react";
import { NativeSelect } from "@/components/native-select";
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
import { Slider } from "@/components/ui/slider";
import { useOpenEntity } from "@/features/entities/use-open-entity";
import type { Climate, GenerationScope, GeneratorParams, LandShape } from "@/lib/bindings";
import { useDebouncedValue } from "@/lib/use-debounced-value";
import { GENERATE_PHASE_LABELS, generateFraction } from "./generate-state";
import { startGeneration, useGenerateStore } from "./generate-store";
import { usePreviewGeneration } from "./queries";
import {
  CLIMATES,
  CUSTOM_SIZE,
  defaultParams,
  formatOffset,
  KINGDOMS_MAX,
  KINGDOMS_MIN,
  LAND_MAX,
  LAND_MIN,
  LAND_SHAPES,
  paramsProblem,
  parseSeed,
  parseSize,
  presetFor,
  PREVIEW_DEBOUNCE_MS,
  rerollSeed,
  SCOPES,
  SEED_MAX,
  SIZE_PRESETS,
  usesKingdoms,
} from "./wizard-model";

/** Title of the wizard when it generates into an existing (empty) map. */
export const TARGET_TITLE = "Generate terrain for this map";

/**
 * Mounted once in the world layout; opened from the gear menu, the palette,
 * the welcome page and an empty map's "Generate terrain…" button (which sets
 * `targetMapEntityId`, so the terrain goes into that map).
 */
export function GenerateWizard() {
  const open = useGenerateStore((s) => s.dialogOpen);
  const setOpen = useGenerateStore((s) => s.setDialogOpen);
  const openCount = useGenerateStore((s) => s.openCount);
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-3xl">
        {/* Fresh settings (and a new random seed) every time the wizard is opened anew. */}
        <WizardBody key={openCount} onClose={() => setOpen(false)} />
      </DialogContent>
    </Dialog>
  );
}

function WizardBody({ onClose }: { onClose: () => void }) {
  const run = useGenerateStore((s) => s.state);
  if (run.status !== "idle") {
    return (
      <RunView
        onClose={onClose}
        onBack={() => useGenerateStore.getState().dispatch({ type: "reset" })}
      />
    );
  }
  return <SettingsView onClose={onClose} />;
}

function Field(props: { label: ReactNode; htmlFor: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <Label htmlFor={props.htmlFor}>{props.label}</Label>
      {props.children}
    </div>
  );
}

const SCOPE_HINTS: Record<GenerationScope, string> = {
  mapOnly:
    "Choose “Map + places” to add settlements and roads, or “Map + places + history” for kingdoms and their history.",
  places: "Kingdoms = number of capital cities (places).",
  history: "Kingdoms = number of realms, each with a capital, a ruling house and its history.",
};

const SEED_PROBLEM = `Seed must be a whole number from 0 to ${SEED_MAX}`;

function SettingsView({ onClose }: { onClose: () => void }) {
  // `useState(() => ...)` runs the initialiser once, so the random seed is picked only on open.
  const [base, setBase] = useState<GeneratorParams>(() => {
    const store = useGenerateStore.getState();
    return store.lastParams ?? defaultParams(undefined, store.lastScope ?? "mapOnly");
  });
  const [seedText, setSeedText] = useState(() => String(base.seed));
  const [sizeChoice, setSizeChoice] = useState(() => presetFor(base.width, base.height));
  const [widthText, setWidthText] = useState(() => String(base.width));
  const [heightText, setHeightText] = useState(() => String(base.height));
  const [kingdomsText, setKingdomsText] = useState(() => String(base.kingdoms));
  const [name, setName] = useState("");
  // Read once: the target only changes when the wizard is opened afresh (new key).
  const [target] = useState(() => useGenerateStore.getState().targetMapEntityId);

  const seed = parseSeed(seedText);
  const preset = SIZE_PRESETS.find((p) => p.id === sizeChoice);
  const width = preset ? preset.width : parseSize(widthText);
  const height = preset ? preset.height : parseSize(heightText);
  const kingdomsEnabled = usesKingdoms(base.scope);
  // Map only ignores the field (it is disabled), so keep the last valid number.
  const kingdoms = kingdomsEnabled ? parseSize(kingdomsText) : base.kingdoms;

  // Memoised so the debounced preview only restarts when a setting really changes.
  const params = useMemo<GeneratorParams>(
    () => ({ ...base, seed: seed ?? -1, width, height, kingdoms }),
    [base, seed, width, height, kingdoms],
  );
  const problem = seed === null ? SEED_PROBLEM : paramsProblem(params);
  const debounced = useDebouncedValue(problem ? null : params, PREVIEW_DEBOUNCE_MS);
  const preview = usePreviewGeneration(debounced);

  const update = (patch: Partial<GeneratorParams>) => setBase((b) => ({ ...b, ...patch }));
  const reroll = () => setSeedText(String(rerollSeed(seed ?? 0)));

  function chooseSize(value: string) {
    setSizeChoice(value);
    const chosen = SIZE_PRESETS.find((p) => p.id === value);
    if (chosen) {
      setWidthText(String(chosen.width));
      setHeightText(String(chosen.height));
    }
  }

  function changeKingdoms(text: string) {
    setKingdomsText(text);
    const value = parseSize(text);
    if (Number.isInteger(value) && value >= KINGDOMS_MIN && value <= KINGDOMS_MAX) {
      update({ kingdoms: value });
    }
  }

  function chooseScope(scope: GenerationScope) {
    update({ scope });
    // Back to map only: show the number that will actually be sent.
    if (!usesKingdoms(scope)) setKingdomsText(String(base.kingdoms));
  }

  function generate() {
    if (problem) return;
    // Generating into an existing map keeps that map's name.
    void startGeneration(params, target ? null : name.trim() || null, target);
  }

  return (
    <>
      <DialogHeader>
        <DialogTitle>{target ? TARGET_TITLE : "Generate map"}</DialogTitle>
        <DialogDescription>
          {target
            ? "Fills this map with land, sea, rivers and biome stamps (and, with places, settlements and roads; with history, kingdoms and their past)."
            : "Creates a new map with land, sea, rivers and biome stamps (and, with places, settlements and roads; with history, kingdoms and their past)."}{" "}
          The same seed and settings always give the same map.
        </DialogDescription>
      </DialogHeader>

      <div className="grid gap-6 sm:grid-cols-[1fr_minmax(0,384px)]">
        <div className="flex flex-col gap-4">
          {!target && (
            <Field label="Map name" htmlFor="generate-name">
              <Input
                id="generate-name"
                value={name}
                placeholder={seed === null ? "Generated map" : `Generated map ${seed}`}
                onChange={(e) => setName(e.target.value)}
              />
            </Field>
          )}

          <Field label="Seed" htmlFor="generate-seed">
            <div className="flex gap-2">
              <Input
                id="generate-seed"
                inputMode="numeric"
                value={seedText}
                aria-invalid={seed === null}
                onChange={(e) => setSeedText(e.target.value)}
              />
              <Button
                type="button"
                variant="outline"
                size="icon"
                aria-label="Random seed"
                title="Random seed"
                onClick={reroll}
              >
                <Dices />
              </Button>
            </div>
          </Field>

          <Field label="Map size" htmlFor="generate-size">
            <NativeSelect
              id="generate-size"
              value={sizeChoice}
              onChange={(e) => chooseSize(e.target.value)}
            >
              {SIZE_PRESETS.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.label}
                </option>
              ))}
              <option value={CUSTOM_SIZE}>Custom…</option>
            </NativeSelect>
          </Field>
          {sizeChoice === CUSTOM_SIZE && (
            <div className="grid grid-cols-2 gap-2">
              <Field label="Width (px)" htmlFor="generate-width">
                <Input
                  id="generate-width"
                  inputMode="numeric"
                  value={widthText}
                  onChange={(e) => setWidthText(e.target.value)}
                />
              </Field>
              <Field label="Height (px)" htmlFor="generate-height">
                <Input
                  id="generate-height"
                  inputMode="numeric"
                  value={heightText}
                  onChange={(e) => setHeightText(e.target.value)}
                />
              </Field>
            </div>
          )}

          <Field label="Land shape" htmlFor="generate-land-shape">
            <NativeSelect
              id="generate-land-shape"
              value={base.landShape}
              onChange={(e) => update({ landShape: e.target.value as LandShape })}
            >
              {LAND_SHAPES.map((s) => (
                <option key={s.value} value={s.value}>
                  {s.label}
                </option>
              ))}
            </NativeSelect>
          </Field>

          <SliderField
            id="generate-land"
            label="Land"
            valueText={`${base.landPercent} %`}
            min={LAND_MIN}
            max={LAND_MAX}
            step={1}
            value={base.landPercent}
            onChange={(landPercent) => update({ landPercent })}
          />

          <Field label="Climate" htmlFor="generate-climate">
            <NativeSelect
              id="generate-climate"
              value={base.climate}
              onChange={(e) => update({ climate: e.target.value as Climate })}
            >
              {CLIMATES.map((c) => (
                <option key={c.value} value={c.value}>
                  {c.label}
                </option>
              ))}
            </NativeSelect>
          </Field>

          <SliderField
            id="generate-temperature"
            label="Temperature"
            valueText={formatOffset(base.temperature)}
            min={-1}
            max={1}
            step={0.1}
            value={base.temperature}
            onChange={(t) => update({ temperature: Math.round(t * 10) / 10 })}
          />
          <SliderField
            id="generate-rainfall"
            label="Rainfall"
            valueText={formatOffset(base.rainfall)}
            min={-1}
            max={1}
            step={0.1}
            value={base.rainfall}
            onChange={(r) => update({ rainfall: Math.round(r * 10) / 10 })}
          />

          <div className="grid grid-cols-2 gap-2">
            <Field label="Scope" htmlFor="generate-scope">
              <NativeSelect
                id="generate-scope"
                value={base.scope}
                onChange={(e) => chooseScope(e.target.value as GenerationScope)}
              >
                {SCOPES.map((s) => (
                  <option key={s.value} value={s.value}>
                    {s.label}
                  </option>
                ))}
              </NativeSelect>
            </Field>
            <Field label="Kingdoms" htmlFor="generate-kingdoms">
              <Input
                id="generate-kingdoms"
                inputMode="numeric"
                value={kingdomsText}
                disabled={!kingdomsEnabled}
                aria-invalid={kingdomsEnabled && !Number.isInteger(kingdoms)}
                onChange={(e) => changeKingdoms(e.target.value)}
              />
            </Field>
          </div>
          <p id="generate-scope-hint" className="-mt-2 text-xs text-muted-foreground">
            {SCOPE_HINTS[base.scope] ?? SCOPE_HINTS.mapOnly}
          </p>
        </div>

        <div className="flex flex-col gap-2">
          <div
            id="generate-preview"
            className="relative flex aspect-[4/3] items-center justify-center overflow-hidden rounded-lg border bg-muted"
          >
            {preview.data ? (
              <img
                src={preview.data}
                alt="Terrain preview"
                className="h-full w-full object-contain"
              />
            ) : (
              !preview.isFetching && (
                <span className="text-sm text-muted-foreground">No preview</span>
              )
            )}
            {preview.isFetching && (
              <Loader2
                aria-label="Rendering preview"
                className="absolute top-2 right-2 size-4 animate-spin text-muted-foreground"
              />
            )}
          </div>
          <Button id="generate-reroll" type="button" variant="outline" onClick={reroll}>
            <RefreshCw /> Reroll
          </Button>
          {preview.error && <p className="text-xs text-destructive">{preview.error.message}</p>}
        </div>
      </div>

      {problem && (
        <p role="alert" className="text-sm text-destructive">
          {problem}
        </p>
      )}

      <DialogFooter>
        <Button variant="outline" onClick={onClose}>
          Cancel
        </Button>
        <Button id="generate-submit" onClick={generate} disabled={problem !== null}>
          Generate
        </Button>
      </DialogFooter>
    </>
  );
}

function SliderField(props: {
  id: string;
  label: string;
  valueText: string;
  min: number;
  max: number;
  step: number;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <Label htmlFor={props.id}>{props.label}</Label>
        <span className="text-sm text-muted-foreground tabular-nums">{props.valueText}</span>
      </div>
      <Slider
        id={props.id}
        aria-label={props.label}
        min={props.min}
        max={props.max}
        step={props.step}
        value={[props.value]}
        onValueChange={([v]) => v !== undefined && props.onChange(v)}
      />
    </div>
  );
}

function Stat({ id, label, value }: { id?: string; label: string; value: ReactNode }) {
  return (
    <div id={id} className="rounded-md border px-3 py-2">
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className="font-medium tabular-nums">{value}</dd>
    </div>
  );
}

function RunView({ onClose, onBack }: { onClose: () => void; onBack: () => void }) {
  const run = useGenerateStore((s) => s.state);
  // The running (or finished) job's target; `null` when it made a new map.
  const target = useGenerateStore((s) => s.targetMapEntityId);
  const openEntity = useOpenEntity();
  const fraction = generateFraction(run);
  const stats = run.result?.stats;
  const settlements = stats?.settlements ?? 0;
  const roads = stats?.roads ?? 0;
  const kingdoms = stats?.kingdoms ?? 0;
  const running = run.status === "running";

  function openMap() {
    if (!run.result) return;
    openEntity(run.result.mapEntityId);
    onClose();
  }

  return (
    <>
      <DialogHeader>
        <DialogTitle>{target ? TARGET_TITLE : "Generate map"}</DialogTitle>
        <DialogDescription>
          {running
            ? "Generating. You can hide this window; the map keeps generating."
            : run.status === "finished"
              ? "The map is ready."
              : "The map could not be generated. Nothing was changed."}
        </DialogDescription>
      </DialogHeader>

      <div className="flex flex-col gap-3" aria-live="polite">
        {running && (
          <>
            <div
              id="generate-progress"
              role="progressbar"
              aria-label="Generation progress"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={Math.round(fraction * 100)}
              className="h-2 w-full overflow-hidden rounded-full bg-muted"
            >
              <div
                className="h-full bg-primary transition-all"
                style={{ width: `${fraction * 100}%` }}
              />
            </div>
            <p id="generate-phase" className="text-sm text-muted-foreground">
              {GENERATE_PHASE_LABELS[run.phase]}
              {run.phase === "writing" && run.total > 0 && ` ${run.done} / ${run.total}`}
            </p>
          </>
        )}
        {run.status === "finished" && stats && (
          <section
            id="generate-result"
            aria-label="Generation result"
            className="flex flex-col gap-2"
          >
            <p role="status" className="text-sm">
              Generated a map with {stats.landMasses}{" "}
              {stats.landMasses === 1 ? "land mass" : "land masses"}
              {settlements > 0 &&
                ` and ${settlements} ${settlements === 1 ? "settlement" : "settlements"}`}
              .
            </p>
            <dl className="grid grid-cols-2 gap-2 text-sm sm:grid-cols-4">
              <Stat label="Land" value={`${stats.landPercentActual} %`} />
              <Stat label="Land masses" value={stats.landMasses} />
              <Stat label="Rivers" value={stats.rivers} />
              <Stat label="Stamps" value={stats.stamps} />
              {settlements > 0 && (
                <>
                  <Stat id="generate-result-settlements" label="Settlements" value={settlements} />
                  <Stat id="generate-result-roads" label="Roads" value={roads} />
                </>
              )}
              {kingdoms > 0 && (
                <>
                  <Stat id="generate-result-kingdoms" label="Kingdoms" value={kingdoms} />
                  <Stat
                    id="generate-result-characters"
                    label="Characters"
                    value={stats.characters ?? 0}
                  />
                  <Stat id="generate-result-events" label="Events" value={stats.events ?? 0} />
                  <Stat
                    id="generate-result-documents"
                    label="Lore documents"
                    value={stats.documents ?? 0}
                  />
                </>
              )}
            </dl>
            {settlements > 0 && (
              <p id="generate-result-places-hint" className="text-xs text-muted-foreground">
                Locations are in the sidebar under Locations; pins open them.
              </p>
            )}
            {kingdoms > 0 && (
              <p id="generate-result-history-hint" className="text-xs text-muted-foreground">
                Factions, characters and events are in the sidebar; open a kingdom to see its ruler,
                capital and lore. The Timeline and Graph show the history.
              </p>
            )}
          </section>
        )}
        {run.status === "failed" && (
          <p role="alert" className="text-sm text-destructive">
            {run.error}
          </p>
        )}
      </div>

      <DialogFooter>
        {/* A filled target map can't take a second generation, so no "Generate another". */}
        {!running && !(target && run.status === "finished") && (
          <Button variant="outline" onClick={onBack}>
            {run.status === "finished" ? "Generate another" : "Back"}
          </Button>
        )}
        <Button
          variant={run.status === "finished" && !target ? "outline" : "default"}
          onClick={onClose}
        >
          {running ? "Hide" : "Close"}
        </Button>
        {/* With a target we are already on the map, so there is nothing to open. */}
        {run.status === "finished" && !target && (
          <Button id="generate-open-map" onClick={openMap}>
            Open map
          </Button>
        )}
      </DialogFooter>
    </>
  );
}
