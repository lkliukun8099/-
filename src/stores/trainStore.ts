// 训练域状态：变式流式生成 + 今日训练作答反馈（§9-5：只存会话态）
import { create } from "zustand";
import { variantApi } from "../api/variant";
import type { AttemptFeedback, VariantDraft, VariantQuestion } from "../types";

interface TrainState {
  /** 流式生成中的草稿（逐条到达） */
  drafts: VariantDraft[];
  generating: boolean;
  /** 今日训练队列（校验通过的变式题） */
  queue: VariantQuestion[];
  queueLoading: boolean;
  /** 各题作答反馈（variantId → feedback） */
  feedbacks: Record<number, AttemptFeedback>;
  submitting: boolean;

  generate: (mistakeId: number, count: number) => Promise<void>;
  clearDrafts: () => void;
  loadQueue: () => Promise<void>;
  submitAnswer: (
    variantId: number,
    answer: string,
    durationMs?: number
  ) => Promise<AttemptFeedback | null>;
}

export const useTrainStore = create<TrainState>((set, get) => ({
  drafts: [],
  generating: false,
  queue: [],
  queueLoading: false,
  feedbacks: {},
  submitting: false,

  generate: async (mistakeId, count) => {
    set({ generating: true, drafts: [] });
    try {
      await variantApi.generate(mistakeId, count, (draft) => {
        // 流式增量：追加到末尾（§9-5：低频追加，避免高频 setState 抖动）
        set((s) => ({ drafts: [...s.drafts, draft] }));
      });
    } finally {
      set({ generating: false });
    }
  },

  clearDrafts: () => set({ drafts: [] }),

  loadQueue: async () => {
    set({ queueLoading: true });
    try {
      const queue = await variantApi.trainQueue(20);
      set({ queue });
    } finally {
      set({ queueLoading: false });
    }
  },

  submitAnswer: async (variantId, answer, durationMs) => {
    set({ submitting: true });
    try {
      const feedback = await variantApi.submitAttempt(variantId, answer, durationMs);
      set((s) => ({ feedbacks: { ...s.feedbacks, [variantId]: feedback } }));
      return feedback;
    } catch {
      return null; // 统一弹错已由 client 处理
    } finally {
      set({ submitting: false });
    }
  },
}));
