import { useState } from "react";
import { useRouter } from "@tanstack/react-router";
import { ArrowLeft, Moon, RefreshCw, Sun, SunMoon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Slider } from "@/components/ui/slider";
import { runUpdateCheck } from "@/features/updates/check-for-updates";
import { SHORTCUTS } from "@/features/shortcuts/matcher";
import { useUiStore, type Theme } from "@/stores/ui-store";
import { useSettingsValue, useUpdateSettings, type Settings } from "./queries";

const THEMES: { value: Theme; label: string; icon: typeof Sun }[] = [
  { value: "light", label: "Light", icon: Sun },
  { value: "dark", label: "Dark", icon: Moon },
  { value: "system", label: "System", icon: SunMoon },
];

// Keep in sync with the clamping in the Rust settings service.
const DELAY_MIN = 300;
const DELAY_MAX = 10000;
const FONT_MIN = 12;
const FONT_MAX = 24;

/** "Ctrl" on Windows/Linux, the Command symbol on macOS. */
function modifierLabel(): string {
  return /mac/i.test(navigator.platform) ? "⌘" : "Ctrl";
}

/** `/settings` - every change is saved immediately. */
export function SettingsPage() {
  const router = useRouter();
  const settings = useSettingsValue();
  const updateSettings = useUpdateSettings();
  const theme = useUiStore((s) => s.theme);
  const setTheme = useUiStore((s) => s.setTheme);

  // While dragging a slider we show its live value but only save on release,
  // so we don't send a command for every pixel of movement.
  const [delayDraft, setDelayDraft] = useState<number | null>(null);
  const [fontDraft, setFontDraft] = useState<number | null>(null);
  const delay = delayDraft ?? settings.autosaveDelayMs;
  const fontSize = fontDraft ?? settings.editorFontSize;

  function save(patch: Partial<Settings>) {
    updateSettings.mutate({ ...settings, ...patch });
  }

  function chooseTheme(next: Theme) {
    setTheme(next); // instant, and keeps the localStorage cache current
    save({ theme: next });
  }

  const [checking, setChecking] = useState(false);
  async function checkNow() {
    setChecking(true);
    await runUpdateCheck({ manual: true });
    setChecking(false);
  }

  const mod = modifierLabel();

  return (
    <div className="flex flex-1 justify-center overflow-auto p-6">
      <div className="flex w-full max-w-xl flex-col gap-8">
        <header className="flex items-center gap-3">
          <Button
            variant="ghost"
            size="icon"
            aria-label="Back"
            onClick={() => router.history.back()}
          >
            <ArrowLeft />
          </Button>
          <h1 className="text-2xl font-semibold">Settings</h1>
        </header>

        <section className="flex flex-col gap-3" aria-labelledby="theme-label">
          <Label id="theme-label">Theme</Label>
          <div role="radiogroup" aria-labelledby="theme-label" className="flex gap-2">
            {THEMES.map(({ value, label, icon: Icon }) => (
              <Button
                key={value}
                role="radio"
                aria-checked={theme === value}
                variant={theme === value ? "default" : "outline"}
                onClick={() => chooseTheme(value)}
              >
                <Icon /> {label}
              </Button>
            ))}
          </div>
        </section>

        <section className="flex flex-col gap-3">
          <div className="flex items-baseline justify-between">
            <Label htmlFor="autosave-delay">Autosave delay</Label>
            <span className="text-sm text-muted-foreground tabular-nums">
              {(delay / 1000).toFixed(1)} s
            </span>
          </div>
          <Slider
            id="autosave-delay"
            aria-label="Autosave delay"
            min={DELAY_MIN}
            max={DELAY_MAX}
            step={100}
            value={[delay]}
            onValueChange={([v]) => v !== undefined && setDelayDraft(v)}
            onValueCommit={([v]) => {
              if (v !== undefined) save({ autosaveDelayMs: v });
              setDelayDraft(null);
            }}
          />
          <p className="text-xs text-muted-foreground">
            How long to wait after you stop typing before saving. Press {mod}+S to save instantly.
          </p>
        </section>

        <section className="flex flex-col gap-3">
          <div className="flex items-baseline justify-between">
            <Label htmlFor="font-size">Editor font size</Label>
            <span className="text-sm text-muted-foreground tabular-nums">{fontSize} px</span>
          </div>
          <Slider
            id="font-size"
            aria-label="Editor font size"
            min={FONT_MIN}
            max={FONT_MAX}
            step={1}
            value={[fontSize]}
            onValueChange={([v]) => v !== undefined && setFontDraft(v)}
            onValueCommit={([v]) => {
              if (v !== undefined) save({ editorFontSize: v });
              setFontDraft(null);
            }}
          />
          <p
            data-testid="font-preview"
            className="rounded-md border p-3"
            style={{ fontSize: fontSize + "px" }}
          >
            The Shattered Coast lay silent beneath a pale and restless sky.
          </p>
        </section>

        <section className="flex flex-col gap-3">
          <h2 className="text-sm font-medium">Updates</h2>
          <div className="flex items-center gap-3">
            <Button variant="outline" onClick={() => void checkNow()} disabled={checking}>
              <RefreshCw /> {checking ? "Checking…" : "Check for updates"}
            </Button>
            <p className="text-xs text-muted-foreground">
              Checks GitHub Releases. Orbis Scriptor also checks quietly when it starts.
            </p>
          </div>
        </section>

        <section className="flex flex-col gap-3">
          <h2 className="text-sm font-medium">Keyboard shortcuts</h2>
          <dl className="flex flex-col divide-y rounded-md border text-sm">
            {SHORTCUTS.map((s) => (
              <div
                key={`${s.action}-${s.key}`}
                className="flex items-center justify-between px-3 py-2"
              >
                <dt>{s.description}</dt>
                <dd>
                  <kbd className="rounded border bg-muted px-1.5 py-0.5 font-mono text-xs text-muted-foreground">
                    {mod}+{s.shift ? "Shift+" : ""}
                    {s.key}
                  </kbd>
                </dd>
              </div>
            ))}
          </dl>
        </section>
      </div>
    </div>
  );
}
