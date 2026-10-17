import { useEffect } from "react";
import { runUpdateCheck } from "./check-for-updates";

/**
 * Checks for an update once, a few seconds after launch, in release builds only
 * (in `tauri dev` there is nothing to update to). Mount it once, near the root.
 */
export function useStartupUpdateCheck() {
  useEffect(() => {
    if (!import.meta.env.PROD) return;
    const timer = setTimeout(() => void runUpdateCheck({ manual: false }), 5000);
    return () => clearTimeout(timer);
  }, []);
}
