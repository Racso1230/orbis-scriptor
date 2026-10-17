import { useEffect } from "react";
import { useSettings } from "@/features/settings/queries";
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
 *
 * The persisted app settings are the source of truth. The UI store (backed by
 * localStorage) is only a cache, so the right theme is painted immediately
 * on startup, before the settings have come back from Rust.
 */
export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const theme = useUiStore((s) => s.theme);
  const setTheme = useUiStore((s) => s.setTheme);
  const settings = useSettings().data;

  // Adopt the saved theme (and refresh the cache) once settings are loaded.
  const savedTheme = settings?.theme;
  useEffect(() => {
    if (savedTheme) setTheme(savedTheme);
  }, [savedTheme, setTheme]);

  // Editor font size is exposed as a CSS variable (see src/index.css).
  const fontSize = settings?.editorFontSize;
  useEffect(() => {
    if (fontSize) document.documentElement.style.setProperty("--editor-font-size", `${fontSize}px`);
  }, [fontSize]);

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
