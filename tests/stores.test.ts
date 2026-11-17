// stores 状态流转测试（api 层整体 mock，验证 store 状态机行为）
import { describe, it, expect, vi, beforeEach } from "vitest";

// ---- mock 全部 api 模块（store 的唯一外部依赖） ----
vi.mock("../src/api/mistake", () => ({
  mistakeApi: {
    list: vi.fn(),
    get: vi.fn(),
    delete: vi.fn(),
    tagList: vi.fn(),
  },
}));
vi.mock("../src/api/variant", () => ({
  variantApi: {
    generate: vi.fn(),
    trainQueue: vi.fn(),
    submitAttempt: vi.fn(),
  },
}));
vi.mock("../src/api/mastery", () => ({
  masteryApi: { report: vi.fn() },
}));

import { mistakeApi } from "../src/api/mistake";
import { variantApi } from "../src/api/variant";
import { masteryApi } from "../src/api/mastery";
import { useMistakeStore } from "../src/stores/mistakeStore";
import { useTrainStore } from "../src/stores/trainStore";
import { useReportStore } from "../src/stores/reportStore";

const mockedList = vi.mocked(mistakeApi.list);
const mockedGet = vi.mocked(mistakeApi.get);
const mockedDelete = vi.mocked(mistakeApi.delete);
const mockedGenerate = vi.mocked(variantApi.generate);
const mockedSubmit = vi.mocked(variantApi.submitAttempt);
const mockedReport = vi.mocked(masteryApi.report);

beforeEach(() => {
  vi.clearAllMocks();
  useMistakeStore.setState({
    query: { page: 1, pageSize: 20 },
    pageData: null,
    loading: false,
    detail: null,
    detailLoading: false,
    tags: [],
  });
  useTrainStore.setState({
    drafts: [],
    generating: false,
    queue: [],
    queueLoading: false,
    feedbacks: {},
    submitting: false,
  });
  useReportStore.setState({ report: null, rangeDays: 30, loading: false });
});

describe("mistakeStore", () => {
  it("setQuery 修改筛选条件时重置到第一页并触发拉取", async () => {
    mockedList.mockResolvedValue({ items: [], total: 0, page: 1, pageSize: 20 });
    useMistakeStore.setState({ query: { page: 5, pageSize: 20 } });
    useMistakeStore.getState().setQuery({ status: "analyzed" });
    const q = useMistakeStore.getState().query;
    expect(q.page).toBe(1);
    expect(q.status).toBe("analyzed");
    await vi.waitFor(() => expect(mockedList).toHaveBeenCalledTimes(1));
  });

  it("setQuery 显式传 page 时不重置", () => {
    mockedList.mockResolvedValue({ items: [], total: 0, page: 3, pageSize: 20 });
    useMistakeStore.getState().setQuery({ page: 3 });
    expect(useMistakeStore.getState().query.page).toBe(3);
  });

  it("fetchList 成功：loading 复位并写入分页数据", async () => {
    const page = { items: [{ id: 1 }], total: 1, page: 1, pageSize: 20 };
    mockedList.mockResolvedValue(page as never);
    await useMistakeStore.getState().fetchList();
    const s = useMistakeStore.getState();
    expect(s.loading).toBe(false);
    expect(s.pageData).toEqual(page);
  });

  it("fetchList 失败：loading 也复位（finally）", async () => {
    mockedList.mockRejectedValue({ code: 2001, message: "db error" });
    await expect(useMistakeStore.getState().fetchList()).rejects.toBeTruthy();
    expect(useMistakeStore.getState().loading).toBe(false);
  });

  it("fetchDetail / clearDetail 流转", async () => {
    mockedGet.mockResolvedValue({ mistake: { id: 7 } } as never);
    await useMistakeStore.getState().fetchDetail(7);
    expect(useMistakeStore.getState().detail?.mistake.id).toBe(7);
    useMistakeStore.getState().clearDetail();
    expect(useMistakeStore.getState().detail).toBeNull();
  });

  it("remove 删除后自动刷新列表", async () => {
    mockedDelete.mockResolvedValue(undefined);
    mockedList.mockResolvedValue({ items: [], total: 0, page: 1, pageSize: 20 });
    await useMistakeStore.getState().remove(9);
    expect(mockedDelete).toHaveBeenCalledWith(9);
    expect(mockedList).toHaveBeenCalled();
  });
});

describe("trainStore", () => {
  it("generate 流式草稿逐条追加，结束后 generating 复位", async () => {
    const draft = (n: number) => ({
      content: `题${n}`,
      contentLatex: null,
      answer: "1",
      solution: null,
      difficulty: 3,
      variantType: "isomorphic",
      validateStatus: "passed" as const,
      variantId: n,
    });
    mockedGenerate.mockImplementation(async (_id, _count, onDraft) => {
      onDraft(draft(1));
      onDraft(draft(2));
    });
    await useTrainStore.getState().generate(1, 2);
    const s = useTrainStore.getState();
    expect(s.drafts).toHaveLength(2);
    expect(s.drafts[0].variantId).toBe(1);
    expect(s.generating).toBe(false);
  });

  it("generate 失败：generating 复位且草稿清空重开", async () => {
    mockedGenerate.mockRejectedValue({ code: 4001, message: "全链路不可用" });
    await expect(useTrainStore.getState().generate(1, 2)).rejects.toBeTruthy();
    expect(useTrainStore.getState().generating).toBe(false);
  });

  it("submitAnswer 成功写入 feedbacks", async () => {
    const fb = { isCorrect: true, feedback: "ok", masteryDelta: 0.15 };
    mockedSubmit.mockResolvedValue(fb);
    const r = await useTrainStore.getState().submitAnswer(42, "x=1", 3000);
    expect(r).toEqual(fb);
    expect(useTrainStore.getState().feedbacks[42]).toEqual(fb);
    expect(useTrainStore.getState().submitting).toBe(false);
  });

  it("submitAnswer 失败返回 null 且不崩溃（统一弹错由 client 负责）", async () => {
    mockedSubmit.mockRejectedValue({ code: 1001, message: "作答不能为空" });
    const r = await useTrainStore.getState().submitAnswer(42, "");
    expect(r).toBeNull();
    expect(useTrainStore.getState().submitting).toBe(false);
    expect(useTrainStore.getState().feedbacks[42]).toBeUndefined();
  });
});

describe("reportStore", () => {
  it("setRangeDays 触发重新拉取", async () => {
    mockedReport.mockResolvedValue({ heatmap: [] } as never);
    useReportStore.getState().setRangeDays(7);
    expect(useReportStore.getState().rangeDays).toBe(7);
    await vi.waitFor(() => expect(mockedReport).toHaveBeenCalledWith(7));
  });

  it("fetchReport 失败时 loading 复位", async () => {
    mockedReport.mockRejectedValue({ code: 2001, message: "x" });
    await expect(useReportStore.getState().fetchReport()).rejects.toBeTruthy();
    expect(useReportStore.getState().loading).toBe(false);
  });
});
