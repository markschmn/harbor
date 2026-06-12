import { create } from "zustand";
import * as api from "@/services/api";

interface LockState {
  hasPin: boolean;
  locked: boolean;
  /** On launch: lock the app if a PIN is configured. */
  init: () => Promise<void>;
  unlock: () => void;
  setHasPin: (v: boolean) => void;
}

export const useLock = create<LockState>((set) => ({
  hasPin: false,
  locked: false,
  init: async () => {
    try {
      const hasPin = await api.hasAppPin();
      set({ hasPin, locked: hasPin });
    } catch {
      set({ hasPin: false, locked: false });
    }
  },
  unlock: () => set({ locked: false }),
  setHasPin: (hasPin) => set({ hasPin }),
}));
