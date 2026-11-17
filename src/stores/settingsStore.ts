// 设置域状态：模型供应商 / 路由策略 / 成本报表 / 备份 / 复习配置
import { create } from "zustand";
import { modelApi } from "../api/model";
import { settingsApi } from "../api/settings";
import { reviewApi } from "../api/review";
import type {
  BackupInfo,
  CostReport,
  ModelProvider,
  ProviderInput,
  RoutePolicy,
} from "../types";

interface SettingsState {
  providers: ModelProvider[];
  policies: RoutePolicy[];
  costReport: CostReport | null;
  backups: BackupInfo[];
  reviewIntervals: number[];
  loading: boolean;

  fetchProviders: () => Promise<void>;
  fetchPolicies: () => Promise<void>;
  fetchCostReport: (rangeDays?: number) => Promise<void>;
  fetchBackups: () => Promise<void>;
  fetchReviewIntervals: () => Promise<void>;
  upsertProvider: (input: ProviderInput) => Promise<void>;
  deleteProvider: (id: number) => Promise<void>;
  upsertPolicy: (policy: RoutePolicy) => Promise<void>;
  setReviewIntervals: (intervals: number[]) => Promise<void>;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  providers: [],
  policies: [],
  costReport: null,
  backups: [],
  reviewIntervals: [1, 3, 7, 15, 30],
  loading: false,

  fetchProviders: async () => {
    const providers = await modelApi.providerList();
    set({ providers });
  },
  fetchPolicies: async () => {
    const policies = await modelApi.policyList();
    set({ policies });
  },
  fetchCostReport: async (rangeDays = 30) => {
    const costReport = await modelApi.costStats(rangeDays);
    set({ costReport });
  },
  fetchBackups: async () => {
    const backups = await settingsApi.backupList();
    set({ backups });
  },
  fetchReviewIntervals: async () => {
    const reviewIntervals = await reviewApi.getIntervals();
    set({ reviewIntervals });
  },
  upsertProvider: async (input) => {
    await modelApi.providerUpsert(input);
    await get().fetchProviders();
  },
  deleteProvider: async (id) => {
    await modelApi.providerDelete(id);
    await get().fetchProviders();
  },
  upsertPolicy: async (policy) => {
    await modelApi.policyUpsert(policy);
    await get().fetchPolicies();
  },
  setReviewIntervals: async (intervals) => {
    await reviewApi.setIntervals(intervals);
    set({ reviewIntervals: intervals });
  },
}));
