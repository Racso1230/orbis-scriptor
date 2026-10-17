import { Moon, Sun, SunMoon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useSettingsValue, useUpdateSettings } from "@/features/settings/queries";
import { useUiStore, type Theme } from "@/stores/ui-store";

const ORDER: Theme[] = ["system", "light", "dark"];

const ICONS = {
  system: SunMoon,
  light: Sun,
  dark: Moon,
} as const;

/** Cycles system → light → dark. */
export function ThemeToggle() {
  const theme = useUiStore((s) => s.theme);
  const setTheme = useUiStore((s) => s.setTheme);
  const settings = useSettingsValue();
  const updateSettings = useUpdateSettings();
  const Icon = ICONS[theme];

  const next = () => {
    const index = ORDER.indexOf(theme);
    const nextTheme = ORDER[(index + 1) % ORDER.length] ?? "system";
    setTheme(nextTheme); // instant, and keeps the localStorage cache current
    updateSettings.mutate({ ...settings, theme: nextTheme }); // persisted by Rust
  };

  return (
    <Button variant="ghost" size="icon" onClick={next} title={`Theme: ${theme}`}>
      <Icon className="size-4" />
      <span className="sr-only">Toggle theme</span>
    </Button>
  );
}
