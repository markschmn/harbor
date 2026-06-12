import { useEffect } from "react";
import { useUi, type View } from "@/stores/ui";
import { MANAGER_TAB, useSessions } from "@/stores/sessions";

/** Ctrl/Cmd + number → switch the primary view. */
const VIEW_KEYS: Record<string, View> = {
  "1": "connections",
  "2": "transfers",
  "3": "keys",
  "4": "settings",
};

function isEditable(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  return !!el?.closest?.("input, textarea, [contenteditable='true']");
}

/**
 * Global keyboard shortcuts:
 * - `Ctrl/Cmd + 1..4` — switch between Connections / Transfers / Keys / Settings
 * - `Ctrl/Cmd + W`    — close the active session tab
 */
export function useGlobalShortcuts(): void {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      if (!mod) return;
      const key = e.key.toLowerCase();

      if (VIEW_KEYS[key] && !isEditable(e.target)) {
        e.preventDefault();
        useUi.getState().setView(VIEW_KEYS[key]);
        return;
      }

      if (key === "w") {
        // Always swallow Ctrl/Cmd+W so the webview window never closes.
        e.preventDefault();
        const sessions = useSessions.getState();
        if (sessions.activeTab !== MANAGER_TAB) {
          void sessions.close(sessions.activeTab);
        }
      }
    };
    window.addEventListener("keydown", handler, { capture: true });
    return () =>
      window.removeEventListener("keydown", handler, { capture: true });
  }, []);
}
