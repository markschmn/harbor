import { useEffect } from "react";
import { useUpdates } from "@/stores/updates";

/**
 * Silently checks for an update a few seconds after launch (production only).
 * If one is found, the [`UpdateBanner`](../components/UpdateBanner.tsx) prompts
 * the user. Manual checks live in Settings.
 */
export function useUpdater(): void {
  useEffect(() => {
    if (import.meta.env.DEV) return;
    const id = window.setTimeout(() => {
      void useUpdates.getState().check(true);
    }, 3000);
    return () => window.clearTimeout(id);
  }, []);
}
