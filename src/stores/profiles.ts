import { create } from "zustand";
import * as api from "@/services/api";
import type { ProfileDraft, ServerProfile } from "@/types";
import { errorMessage, toast } from "./toast";

interface ProfileState {
  profiles: ServerProfile[];
  query: string;
  selectedId: string | null;
  loading: boolean;
  load: () => Promise<void>;
  setQuery: (q: string) => void;
  select: (id: string | null) => void;
  create: (draft: ProfileDraft) => Promise<ServerProfile | null>;
  update: (id: string, draft: ProfileDraft) => Promise<ServerProfile | null>;
  remove: (id: string) => Promise<void>;
  toggleFavorite: (id: string) => Promise<void>;
}

export const useProfiles = create<ProfileState>((set, get) => ({
  profiles: [],
  query: "",
  selectedId: null,
  loading: false,

  load: async () => {
    set({ loading: true });
    try {
      const profiles = await api.listProfiles();
      set({ profiles });
      // Keep a valid selection.
      const { selectedId } = get();
      if (selectedId && !profiles.some((p) => p.id === selectedId)) {
        set({ selectedId: profiles[0]?.id ?? null });
      } else if (!selectedId && profiles.length) {
        set({ selectedId: profiles[0].id });
      }
    } catch (e) {
      toast("error", errorMessage(e), "Failed to load profiles");
    } finally {
      set({ loading: false });
    }
  },

  setQuery: (query) => set({ query }),
  select: (selectedId) => set({ selectedId }),

  create: async (draft) => {
    try {
      const created = await api.createProfile(draft);
      await get().load();
      set({ selectedId: created.id });
      toast("success", `Saved “${created.name}”`);
      return created;
    } catch (e) {
      toast("error", errorMessage(e), "Could not save profile");
      return null;
    }
  },

  update: async (id, draft) => {
    try {
      const updated = await api.updateProfile(id, draft);
      await get().load();
      toast("success", `Updated “${updated.name}”`);
      return updated;
    } catch (e) {
      toast("error", errorMessage(e), "Could not update profile");
      return null;
    }
  },

  remove: async (id) => {
    try {
      await api.deleteProfile(id);
      await get().load();
      toast("info", "Profile deleted");
    } catch (e) {
      toast("error", errorMessage(e), "Could not delete profile");
    }
  },

  toggleFavorite: async (id) => {
    try {
      await api.toggleFavorite(id);
      await get().load();
    } catch (e) {
      toast("error", errorMessage(e));
    }
  },
}));

export function filteredProfiles(state: ProfileState): ServerProfile[] {
  const q = state.query.trim().toLowerCase();
  if (!q) return state.profiles;
  return state.profiles.filter(
    (p) =>
      p.name.toLowerCase().includes(q) ||
      p.host.toLowerCase().includes(q) ||
      p.username.toLowerCase().includes(q) ||
      p.tags.some((t) => t.toLowerCase().includes(q)),
  );
}
