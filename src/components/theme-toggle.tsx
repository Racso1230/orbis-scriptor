import { Moon, Sun, SunMoon } from "lucide-react";
import { Button } from "@/components/ui/button";
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
  const Icon = ICONS[theme];

  const next = () => {
    const index = ORDER.indexOf(theme);
    setTheme(ORDER[(index + 1) % ORDER.length] ?? "system");
  };

  return (
    <Button variant="ghost" size="icon" onClick={next} title={`Theme: ${theme}`}>
      <Icon className="size-4" />
      <span className="sr-only">Toggle theme</span>
    </Button>
  );
}
