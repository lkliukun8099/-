// 错题域状态：列表筛选/分页 + 详情（架构 §9-5：列表页本地 state 语义内聚在域 store）
import { create } from "zustand";
import { mistakeApi } from "../api/mistake";
import type {
  MistakeDetail,
  MistakeQuery,
  MistakeSummary,
  Page,
  Tag,
} from "../types";

interface MistakeState {
  query: MistakeQuery;
  pageData: Page<MistakeSummary> | null;
  loading: boolean;
  detail: MistakeDetail | null;
  detailLoading: boolean;
  tags: Tag[];
  setQuery: (patch: Partial<MistakeQuery>) => void;
  fetchList: () => Promise<void>;
  fetchDetail: (id: number) => Promise<void>;
  clearDetail: () => void;
  fetchTags: () => Promise<void>;
  remove: (id: number) => Promise<void>;
}

const DEFAULT_QUERY: MistakeQuery = { page: 1, pageSize: 20 };

export const useMistakeStore = create<MistakeState>((set, get) => ({
  query: DEFAULT_QUERY,
  pageData: null,
  loading: false,
  detail: null,
  detailLoading: false,
  tags: [],

  setQuery: (patch) => {
    // 改筛选条件时回到第一页
    set((s) => ({ query: { ...s.query, ...patch, page: patch.page ?? 1 } }));
    void get().fetchList();
  },

  fetchList: async () => {
    set({ loading: true });
    try {
      const pageData = await mistakeApi.list(get().query);
      set({ pageData });
    } finally {
      set({ loading: false });
    }
  },

  fetchDetail: async (id) => {
    set({ detailLoading: true });
    try {
      const detail = await mistakeApi.get(id);
      set({ detail });
    } finally {
      set({ detailLoading: false });
    }
  },

  clearDetail: () => set({ detail: null }),

  fetchTags: async () => {
    const tags = await mistakeApi.tagList();
    set({ tags });
  },

  remove: async (id) => {
    await mistakeApi.delete(id);
    await get().fetchList();
  },
}));
