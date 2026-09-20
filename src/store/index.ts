import { create } from 'zustand';
import type { AppSettings, SharedGame, CloudConnectionStatus, ToastMessage, ModalType } from '../types/index';
let tc = 0;
interface AppState {
  settings: AppSettings | null; sharedGames: SharedGame[]; selectedAppId: string | null;
  cloudStatus: CloudConnectionStatus; modal: ModalType; modalData: unknown; toasts: ToastMessage[]; isLoading: boolean;
  setSettings: (s: AppSettings) => void; updateSettings: (p: Partial<AppSettings>) => void;
  setSharedGames: (g: SharedGame[]) => void; updateSharedGame: (id: string, p: Partial<SharedGame>) => void;
  selectGame: (id: string | null) => void; setCloudStatus: (s: CloudConnectionStatus) => void;
  openModal: (t: ModalType, d?: unknown) => void; closeModal: () => void;
  addToast: (m: Omit<ToastMessage, 'id'>) => void; removeToast: (id: string) => void;
  setLoading: (v: boolean) => void;
}
export const useStore = create<AppState>((set) => ({
  settings: null, sharedGames: [], selectedAppId: null,
  cloudStatus: { connected: false, providerType: null }, modal: null, modalData: null, toasts: [], isLoading: false,
  setSettings: (s) => set({ settings: s }),
  updateSettings: (p) => set((st) => ({ settings: st.settings ? { ...st.settings, ...p } : null })),
  setSharedGames: (g) => set((st) => ({
    // Preserve cloudInfo already fetched — backend doesn't return it
    sharedGames: g.map((newGame) => {
      const existing = st.sharedGames.find((e) => e.appId === newGame.appId);
      return existing?.cloudInfo ? { ...newGame, cloudInfo: existing.cloudInfo } : newGame;
    }),
  })),
  updateSharedGame: (id, p) => set((st) => ({ sharedGames: st.sharedGames.map((g) => g.appId === id ? { ...g, ...p } : g) })),
  selectGame: (id) => set({ selectedAppId: id }),
  setCloudStatus: (s) => set({ cloudStatus: s }),
  openModal: (t, d = null) => set({ modal: t, modalData: d }),
  closeModal: () => set({ modal: null, modalData: null }),
  addToast: (m) => {
    const id = String(++tc); const dur = m.duration ?? 4000;
    set((st) => ({ toasts: [...st.toasts, { ...m, id }] }));
    if (dur > 0) setTimeout(() => set((st) => ({ toasts: st.toasts.filter((t) => t.id !== id) })), dur);
  },
  removeToast: (id) => set((st) => ({ toasts: st.toasts.filter((t) => t.id !== id) })),
  setLoading: (v) => set({ isLoading: v }),
}));
