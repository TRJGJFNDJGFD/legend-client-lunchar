import { create } from "zustand";
import { api, JavaInstall, Profile } from "../lib/tauri";

interface AppState {
  javaInstalls: JavaInstall[];
  javaChecked: boolean;
  profiles: Profile[];
  activeProfileId: string | null;

  refreshJava: () => Promise<void>;
  refreshProfiles: () => Promise<void>;
  setActiveProfile: (id: string) => void;
  createProfile: (input: {
    name: string;
    minecraft_version: string;
    loader: string;
    min_ram_mb: number;
    max_ram_mb: number;
  }) => Promise<void>;
  deleteProfile: (id: string) => Promise<void>;
}

export const useAppStore = create<AppState>((set, get) => ({
  javaInstalls: [],
  javaChecked: false,
  profiles: [],
  activeProfileId: null,

  refreshJava: async () => {
    const installs = await api.detectJava();
    set({ javaInstalls: installs, javaChecked: true });
  },

  refreshProfiles: async () => {
    const profiles = await api.listProfiles();
    set({ profiles });
    const current = get().activeProfileId;
    if (!current && profiles.length > 0) {
      set({ activeProfileId: profiles[0].id });
    }
  },

  setActiveProfile: (id) => set({ activeProfileId: id }),

  createProfile: async (input) => {
    const profile = await api.createProfile(input);
    set((state) => ({
      profiles: [...state.profiles, profile],
      activeProfileId: profile.id,
    }));
  },

  deleteProfile: async (id) => {
    await api.deleteProfile(id);
    set((state) => ({
      profiles: state.profiles.filter((p) => p.id !== id),
      activeProfileId: state.activeProfileId === id ? null : state.activeProfileId,
    }));
  },
}));
