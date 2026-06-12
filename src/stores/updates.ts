import { create } from "zustand";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { errorMessage, toast } from "./toast";

interface UpdateState {
  update: Update | null;
  checking: boolean;
  downloading: boolean;
  progress: number; // 0..1
  lastChecked: number | null;
  /** Check the update endpoint. `silent` suppresses the "up to date" toast. */
  check: (silent?: boolean) => Promise<void>;
  /** Download, install and relaunch into the new version. */
  install: () => Promise<void>;
  dismiss: () => void;
}

export const useUpdates = create<UpdateState>((set, get) => ({
  update: null,
  checking: false,
  downloading: false,
  progress: 0,
  lastChecked: null,

  check: async (silent = false) => {
    if (get().checking || get().downloading) return;
    set({ checking: true });
    try {
      const update = await check();
      set({ update: update ?? null, lastChecked: Date.now() });
      if (!update && !silent) {
        toast("success", "Harbor is up to date");
      }
    } catch (e) {
      // A missing release / offline endpoint is expected; only surface on a
      // user-initiated check.
      if (!silent) toast("error", errorMessage(e), "Update check failed");
    } finally {
      set({ checking: false });
    }
  },

  install: async () => {
    const update = get().update;
    if (!update) return;
    set({ downloading: true, progress: 0 });
    try {
      let total = 0;
      let received = 0;
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
        } else if (event.event === "Progress") {
          received += event.data.chunkLength;
          set({ progress: total > 0 ? received / total : 0 });
        }
      });
      // Relaunch into the freshly installed version.
      await relaunch();
    } catch (e) {
      toast("error", errorMessage(e), "Update failed");
      set({ downloading: false });
    }
  },

  dismiss: () => set({ update: null }),
}));
