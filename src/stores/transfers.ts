import { create } from "zustand";
import * as api from "@/services/api";
import type { TransferEvent, TransferTask } from "@/types";
import { errorMessage, toast } from "./toast";

interface TransferState {
  transfers: Record<string, TransferTask>;
  load: () => Promise<void>;
  apply: (event: TransferEvent) => void;
  cancel: (id: string) => Promise<void>;
  retry: (id: string) => Promise<void>;
  clearFinished: () => Promise<void>;
}

export const useTransfers = create<TransferState>((set, get) => ({
  transfers: {},

  load: async () => {
    try {
      const list = await api.listTransfers();
      const map: Record<string, TransferTask> = {};
      for (const t of list) map[t.id] = t;
      set({ transfers: map });
    } catch (e) {
      toast("error", errorMessage(e), "Failed to load transfers");
    }
  },

  apply: (event) =>
    set((s) => {
      const transfers = { ...s.transfers };
      if (event.kind === "added") {
        transfers[event.task.id] = event.task;
      } else if (event.kind === "progress") {
        const t = transfers[event.progress.id];
        if (t) {
          transfers[event.progress.id] = {
            ...t,
            transferred_bytes: event.progress.transferred_bytes,
            total_bytes: event.progress.total_bytes || t.total_bytes,
          };
        }
      } else if (event.kind === "state_changed") {
        const t = transfers[event.id];
        if (t) transfers[event.id] = { ...t, state: event.state };
      }
      return { transfers };
    }),

  cancel: async (id) => {
    try {
      await api.cancelTransfer(id);
    } catch (e) {
      toast("error", errorMessage(e));
    }
  },

  retry: async (id) => {
    try {
      await api.retryTransfer(id);
    } catch (e) {
      toast("error", errorMessage(e));
    }
  },

  clearFinished: async () => {
    try {
      await api.clearFinishedTransfers();
      await get().load();
    } catch (e) {
      toast("error", errorMessage(e));
    }
  },
}));

export function transferList(state: TransferState): TransferTask[] {
  return Object.values(state.transfers).sort(
    (a, b) =>
      new Date(b.created_at).getTime() - new Date(a.created_at).getTime(),
  );
}

export function activeTransferCount(state: TransferState): number {
  return Object.values(state.transfers).filter(
    (t) => t.state.state === "active" || t.state.state === "queued",
  ).length;
}
