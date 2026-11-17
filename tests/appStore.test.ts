// appStore 全局状态测试（Node 环境无 DOM navigator.onLine，先 stub 再动态导入）
import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

async function loadStore() {
  vi.resetModules();
  vi.stubGlobal("navigator", { onLine: true });
  vi.stubGlobal("window", {
    addEventListener: vi.fn(),
    setInterval: vi.fn(),
  });
  return await import("../src/stores/appStore");
}

describe("appStore", () => {
  beforeEach(() => {
    vi.unstubAllGlobals();
  });

  it("初始在线状态来自 navigator.onLine", async () => {
    const { useAppStore } = await loadStore();
    expect(useAppStore.getState().online).toBe(true);
    expect(useAppStore.getState().profileId).toBe(1);
  });

  it("notify / closeNotice", async () => {
    const { useAppStore } = await loadStore();
    useAppStore.getState().notify({ severity: "error", message: "boom" });
    expect(useAppStore.getState().notice?.message).toBe("boom");
    useAppStore.getState().closeNotice();
    expect(useAppStore.getState().notice).toBeNull();
  });

  it("toggleDarkMode 翻转", async () => {
    const { useAppStore } = await loadStore();
    expect(useAppStore.getState().darkMode).toBe(false);
    useAppStore.getState().toggleDarkMode();
    expect(useAppStore.getState().darkMode).toBe(true);
  });

  it("initOnlineListener：后端校准取浏览器与后端双信号交集", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockResolvedValue(false); // 后端探测离线
    const { useAppStore } = await loadStore();
    await useAppStore.getState().initOnlineListener();
    expect(useAppStore.getState().online).toBe(false);
  });

  it("initOnlineListener：后端未就绪时保持浏览器信号", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockRejectedValue(new Error("not ready"));
    const { useAppStore } = await loadStore();
    await useAppStore.getState().initOnlineListener();
    expect(useAppStore.getState().online).toBe(true);
  });
});
