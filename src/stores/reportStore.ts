// 诊断报告数据缓存（F-13）
import { create } from "zustand";
import { masteryApi } from "../api/mastery";
import type { ReportDto } from "../types";

interface ReportState {
  report: ReportDto | null;
  rangeDays: number;
  loading: boolean;
  setRangeDays: (days: number) => void;
  fetchReport: () => Promise<void>;
}

export const useReportStore = create<ReportState>((set, get) => ({
  report: null,
  rangeDays: 30,
  loading: false,

  setRangeDays: (days) => {
    set({ rangeDays: days });
    void get().fetchReport();
  },

  fetchReport: async () => {
    set({ loading: true });
    try {
      const report = await masteryApi.report(get().rangeDays);
      set({ report });
    } finally {
      set({ loading: false });
    }
  },
}));
