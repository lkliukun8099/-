// 全局应用状态：在线状态 / 档案 / 全局通知 / 主题（架构 §9-5/§9-8）
import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

export interface Notice {
  severity: "success" | "info" | "warning" | "error";
  message: string;
}

interface AppState {
  /** 在线状态（浏览器事件 + Gateway 探测双信号） */
  online: boolean;
  darkMode: boolean;
  notice: Notice | null;
  /** 当前档案 ID（F-22 预留，首版固定 1） */
  profileId: number;
  notify: (notice: Notice) => void;
  closeNotice: () => void;
  toggleDarkMode: () => void;
  setOnline: (online: boolean) => void;
  /** 初始化在线状态监听（浏览器 online/offline + 后端状态轮询兜底） */
  initOnlineListener: () => Promise<void>;
}

export const useAppStore = create<AppState>((set, get) => ({
  online: navigator.onLine,
  darkMode: false,
  notice: null,
  profileId: 1,

  notify: (notice) => set({ notice }),
  closeNotice: () => set({ notice: null }),
  toggleDarkMode: () => set((s) => ({ darkMode: !s.darkMode })),
  setOnline: (online) => set({ online }),

  initOnlineListener: async () => {
    window.addEventListener("online", () => set({ online: true }));
    window.addEventListener("offline", () => set({ online: false }));
    // 启动时以后端状态校准一次（后端会综合 Gateway 探测结果）
    try {
      const backendOnline = await invoke<boolean>("network_status");
      // 浏览器信号优先：浏览器离线时必然离线
      set({ online: navigator.onLine && backendOnline });
    } catch {
      // 后端未就绪时保持浏览器信号
      set({ online: navigator.onLine });
    }
    // 每 60s 校准一次
    window.setInterval(() => {
      if (!navigator.onLine && get().online) {
        set({ online: false });
      }
    }, 60_000);
  },
}));
