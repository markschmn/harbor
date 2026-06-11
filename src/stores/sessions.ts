import { create } from "zustand";
import * as api from "@/services/api";
import type { SessionInfo, SessionStatus } from "@/types";
import { errorMessage, toast } from "./toast";

export type SessionPanel = "terminal" | "files";
export const MANAGER_TAB = "manager";

export interface SessionUi {
  info: SessionInfo;
  panel: SessionPanel;
}

interface SessionState {
  sessions: Record<string, SessionUi>;
  order: string[];
  activeTab: string;
  setActiveTab: (tab: string) => void;
  addSession: (info: SessionInfo) => void;
  setPanel: (id: string, panel: SessionPanel) => void;
  setStatus: (id: string, status: SessionStatus) => void;
  close: (id: string) => Promise<void>;
}

export const useSessions = create<SessionState>((set) => ({
  sessions: {},
  order: [],
  activeTab: MANAGER_TAB,

  setActiveTab: (activeTab) => set({ activeTab }),

  addSession: (info) =>
    set((s) => ({
      sessions: { ...s.sessions, [info.id]: { info, panel: "terminal" } },
      order: s.order.includes(info.id) ? s.order : [...s.order, info.id],
      activeTab: info.id,
    })),

  setPanel: (id, panel) =>
    set((s) => {
      const existing = s.sessions[id];
      if (!existing) return s;
      return { sessions: { ...s.sessions, [id]: { ...existing, panel } } };
    }),

  setStatus: (id, status) =>
    set((s) => {
      const existing = s.sessions[id];
      if (!existing) return s;
      return {
        sessions: {
          ...s.sessions,
          [id]: { ...existing, info: { ...existing.info, status } },
        },
      };
    }),

  close: async (id) => {
    try {
      await api.disconnect(id);
    } catch (e) {
      toast("error", errorMessage(e));
    }
    set((s) => {
      const sessions = { ...s.sessions };
      delete sessions[id];
      const order = s.order.filter((x) => x !== id);
      const activeTab =
        s.activeTab === id ? order[order.length - 1] ?? MANAGER_TAB : s.activeTab;
      return { sessions, order, activeTab };
    });
  },
}));
