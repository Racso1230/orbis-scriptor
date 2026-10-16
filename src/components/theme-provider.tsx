import { useEffect } from "react";
import { useUiStore, type Theme } from "@/stores/ui-store";

const DARK_QUERY = "(prefers-color-scheme: dark)";

function resolveIsDark(theme: Theme): boolean {
  if (theme === "system") {
    return window.matchMedia(DARK_QUERY).matches;
  }
  return theme === "dark";
}

function applyTheme(theme: Theme) {
  document.documentElement.classList.toggle("dark", resolveIsDark(theme));
}

/**
 * Keeps the `dark` class on <html> in sync with the theme in the UI store.
 * shadcn/ui's CSS variables switch on that class (see src/index.css).
 */
export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const theme = useUiStore((s) => s.theme);

  useEffect(() => {
    applyTheme(theme);

    // When following the OS, re-apply whenever the OS preference changes.
    if (theme !== "system") return;
    const media = window.matchMedia(DARK_QUERY);
    const onChange = () => applyTheme("system");
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, [theme]);

  return children;
}
