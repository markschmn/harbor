import { create } from "zustand";
import type { AppInfo } from "@/types";

export type View = "connections" | "transfers" | "keys" | "settings";
export type Theme = "dark" | "light";

const THEME_KEY = "harbor.theme";

function initialTheme(): Theme {
  const stored = localStorage.getItem(THEME_KEY);
  if (stored === "light" || stored === "dark") return stored;
  return window.matchMedia?.("(prefers-color-scheme: light)").matches
    ? "light"
    : "dark";
}

function applyTheme(theme: Theme) {
  document.documentElement.setAttribute("data-theme", theme);
}

interface UiState {
  theme: Theme;
  view: View;
  appInfo: AppInfo | null;
  setTheme: (t: Theme) => void;
  toggleTheme: () => void;
  setView: (v: View) => void;
  setAppInfo: (info: AppInfo) => void;
}

export const useUi = create<UiState>((set, get) => ({
  theme: initialTheme(),
  view: "connections",
  appInfo: null,
  setTheme: (theme) => {
    applyTheme(theme);
    localStorage.setItem(THEME_KEY, theme);
    set({ theme });
  },
  toggleTheme: () => get().setTheme(get().theme === "dark" ? "light" : "dark"),
  setView: (view) => set({ view }),
  setAppInfo: (appInfo) => set({ appInfo }),
}));

// Apply the persisted theme as early as possible.
applyTheme(useUi.getState().theme);
